# PocketSwarm MVP Worker Protocol

Status: MVP normative specification  
Wire protocol version: 1.0

本書は、単一のOrchestratorと、事前ペアリングされたWorker daemonとの間の通信意味論を定義する。規範語 MUST、MUST NOT、SHOULD、SHOULD NOT、MAY は、それぞれ必須、禁止、推奨、非推奨、任意を表す。衝突する記述がある場合、より安全側へ停止する要件を優先する。

## 1. 目的と保証範囲

PocketSwarm MVPは、古いAndroid端末（Termux）やRaspberry Piのローカル推論能力を、中央集権型のOrchestratorから安全に利用するためのプロトコルである。

- OrchestratorはTask、Attempt、Lease、Cancellation、および採用済み終端結果の唯一の正本（authoritative source of truth）でなければならない（MUST）。Workerの報告だけで正本を巻き戻したり上書きしたりしてはならない（MUST NOT）。
- Worker daemonは、そのNodeにおける安全判断、単一実行slot、Python Engineの起動・停止・強制終了について最終権限を持たなければならない（MUST）。Orchestratorからの割当であっても、ローカル安全条件を満たさなければ拒否または中断しなければならない（MUST）。
- 配信はat-least-onceを前提とする。送信側は、応答または後続状態が定義された未確認control messageとterminal eventを、意味上の応答、期限切れ、またはfencingで不要と確定するまで同じmessage IDで再送しなければならず（MUST）、受信側は重複を正しく処理しなければならない（MUST）。恒久的なnetwork partitionでの到達は保証しない。表示専用のattempt.progressとattempt.output_chunkだけは、後述するbest-effortの明示的例外である。
- Exactly-once executionは保証しない。ネットワーク分断、ACK消失、再接続、または新Attemptへの再割当によって、同じ論理Taskの計算が複数回行われることがある。
- Orchestratorはauthenticated node ID、Attempt ID、Lease ID、Lease revision、および現在のTask状態をtransaction内で検証し、正本へ採用する終端結果を一つに限定しなければならない（MUST）。古いAttemptまたはfence済みLeaseの結果は採用してはならない（MUST NOT）。
- Taskは外部副作用を持ってはならない（MUST NOT）。at-least-once環境では、送信、決済、ファイル変更、任意コマンド、URL取得などを重複実行しても安全に取り消せないためである。生成コードをテキストとして返すことはあり得るが、Orchestrator、Worker、Engineのいずれもそれを実行してはならない（MUST NOT）。

### 1.1 固定するMVP境界

次の境界はMVPでは変更しない。

- Orchestratorは1台のみとする。
- Workerは事前ペアリング済みとする。
- 利用範囲はローカルネットワーク内とする。
- Worker同士は直接通信してはならない（MUST NOT）。
- Worker一台につき同時推論は最大一件とする。
- 対応Taskは型付きLLM推論だけとする。
- 任意シェルコマンド、生成コードの実行、任意URL取得、任意ファイルアクセスを禁止する。
- モデル転送および自動ダウンロードを行わない。
- OrchestratorのHA、リーダー選出、P2P、動的ペアリングは対象外とする。
- Exactly-once executionを保証しない。
- ブラウザUIはOrchestrator側だけで使用し、Worker側はCUIを使用する。
- 内部思考およびchain-of-thoughtは通信、保存、表示してはならない（MUST NOT）。
- 表示可能な情報は、生成テキスト、処理段階、token数、速度、安全状態などの外部向け情報に限定する。

## 2. 信頼境界

| 境界・主体 | 信頼する範囲 | 信頼しない範囲と必須対策 |
|---|---|---|
| LAN | 到達可能性だけを仮定する | 盗聴、改ざん、偽Discovery、replay、DoSが可能とみなす。Task通信はWSSで保護しなければならない（MUST） |
| Orchestrator | Task正本、スケジューリング、認証済みNodeとの対応を保持する | 侵害時には全prompt/resultが漏えいし得る。Workerのローカル安全hard limitを上書きする権限は持たない |
| Worker daemon | ローカル安全監視、credential保護、Engine監督、単一slot制御を担う | 侵害・改変されたWorkerは温度、性能、メモリ、モデル能力について虚偽報告できる。自己申告はattestationではない |
| Python Engine | インストール済みGGUFモデルによる型付き推論だけを行う | 出力、例外、token数、終了状態を信頼せず、daemonが型・サイズ・Attempt対応を検証する。LANアクセス権限を与えない |
| Workerの自己申告情報 | スケジューリングの参考情報として利用できる | 温度・性能・capabilityの真実性を保証しない。認証済みNode IDと自己申告Node IDを混同してはならない |
| モデル出力 | untrusted plain textとして扱う | 命令、HTML、ANSI、Rich markup、コード、URLを含み得る。実行・fetch・HTML解釈してはならない |
| Dashboard | 認可された管理者が正本を閲覧・操作する入口とする | モデル出力とNode名をuntrusted dataとしてescapeする。Worker credentialとDashboard管理者credentialを共用してはならない |

OrchestratorはTaskのpromptとresultの完全な内容を閲覧できる。利用者へこの機密性境界を明示しなければならない（MUST）。UDP discoveryは認証ではなく、発見のhintにすぎない。Workerのローカルrequired sensor、温度hard limit、cooldown、強制終了期限は、Orchestratorから緩和できてはならない（MUST NOT）。

## 3. Discoveryと接続

### 3.1 UDP discovery

UDPはOrchestrator発見専用とする。discovery.announceへTask、prompt、result、credential、Node一覧、pairing bundleを含めてはならない（MUST NOT）。announcementを受信しただけでNodeを登録済みまたは認証済みと扱ってはならない（MUST NOT）。

MVP discoveryのdefault transportはIPv4 UDP/49161へのLAN-local broadcastとする。Orchestratorは有効な各LAN interfaceのdirected broadcast addressへ一方向にannounceし、Workerは同じportをlistenする。初期announce間隔は5,000 msとするが、運用設定で変更してよい（MAY）。IPv6 multicastとWorkerからのUDP応答はMVP対象外である。default portを変更する場合は両者へ帯域外設定が必要であり、利用できなければ手動URLへfallbackする。

discovery.announceはUTF-8 JSONの単一UDP datagramであり、共通envelopeに準じる。ただしSession確立前であるためsession_idはnullでなければならない（MUST）。payloadは少なくとも次を持つ。

| Field | Type | 意味 |
|---|---|---|
| discovery_version | string | Discovery形式のMAJOR.MINOR |
| cluster_id | UUID | 対象cluster |
| orchestrator_id | UUID | Orchestratorの永続ID |
| worker_wss_url | string | Worker接続用wss URL |
| supported_protocol_versions | string array | 対応Wire protocol version |
| orchestrator_public_key_sha256_pin | string | TLS公開鍵pin |
| valid_for_ms | safe integer | 受信時からの有効時間 |
| nonce | string | announcementごとに生成する128-bit以上のrandom nonce |

Workerは、受信したcluster_id、orchestrator_id、公開鍵pinをpairing bundleと照合しなければならない（MUST）。一致しないannouncementは黙って無視しなければならない（MUST）。valid_for_msはUDP受信時のWorker monotonic clockから測定する。nonceと有効時間は重複・古いannouncementの抑制用であり、送信元認証やreplay耐性の証明として扱ってはならない（MUST NOT）。

discovery.announce以外のtype、malformed JSON、oversized datagramへUDPでprotocol.errorその他の応答を返してはならず（MUST NOT）、黙って破棄する。これはreflection/amplificationを避けるためである。

UDPが利用できない場合、管理者はpairing bundle内のworker_wss_urlまたは明示的な手動Orchestrator URLを使用できる（MAY）。手動URLを使ってもcluster ID、Orchestrator ID、および公開鍵pinの検証を省略してはならない（MUST NOT）。

### 3.2 WSS接続

- WorkerからOrchestratorへ外向きWSS接続を確立しなければならない（MUST）。OrchestratorからWorkerへの着信接続はMVPでは定義しない。
- WorkerはTLS handshakeで公開鍵pinを検証した後に限りAuthorization headerを送らなければならない（MUST）。pin不一致時にcredentialを送ってはならない（MUST NOT）。
- redirectへAuthorization headerを自動転送してはならない（MUST NOT）。
- LAN上の平文WebSocket（ws）は禁止する（MUST NOT）。
- 唯一の例外は、明示的な開発modeで、接続先が正確に127.0.0.1または[::1]であり、本番credentialと本番clusterを使用しない場合である。0.0.0.0、LAN address、hostnameの解決結果をloopback例外として扱ってはならない（MUST NOT）。
- 一つのnode_idに対して有効Sessionは一つだけでなければならない（MUST）。有効な新Sessionを原子的に確立した時点で旧Sessionをfenceし、送信可能ならstale_sessionを通知したうえで旧WebSocketを切断しなければならない（MUST）。切断完了を待たず、fencing commit後の旧messageを拒否する。

## 4. 事前ペアリングと認証

### 4.1 Pairing bundle

MVPのpairing bundleは、信頼できる帯域外手段でWorkerへ導入し、少なくとも次を含めなければならない（MUST）。

| Field | 要件 |
|---|---|
| cluster_id | Orchestratorが管理するclusterの永続UUID |
| orchestrator_id | 単一Orchestratorの永続UUID |
| worker_wss_url | defaultのWorker用WSS URL |
| orchestrator_public_key_sha256_pin | Orchestrator TLS公開鍵のSPKI DERに対するSHA-256 pin。Wire表現はsha256/に64桁のlowercase hexadecimal digestを続ける |
| node_id | 当該Workerに固定されたstable UUID |
| node_credential | Nodeごとに独立した256-bit以上の暗号学的random secret |

node_credentialはopaque値であり、Orchestratorはcredentialから登録済みnode_idを検索できなければならない（MUST）。Workerがsession.helloで自己申告したnode_idは、この認証済みnode_idと一致しなければならない（MUST）。

### 4.2 Authorization

WorkerはWebSocket HTTP upgradeで次の形式を使用する。

~~~text
Authorization: PocketSwarm-Node <base64url-encoded-credential>
~~~

credentialをURL、query string、JSON payload、close reason、診断dump、通常ログへ出してはならない（MUST NOT）。OrchestratorとWorkerはAuthorization headerを必ずredactしなければならない（MUST）。認証失敗はupgrade前に一般化した401または403で拒否し、Nodeの存在を漏らす詳細を返すべきではない（SHOULD NOT）。

### 4.3 失効、rotation、pin更新

- Orchestratorはnode_credentialをNode単位で個別失効できなければならない（MUST）。失効commit後は当該Nodeの現Sessionを切断し、再接続を拒否しなければならない（MUST）。
- credential失効commit時には当該Nodeのcurrent Leaseもrevokeし、以後のresultを正本へ採用してはならない（MUST NOT）。
- credential rotationは新credentialを信頼できる帯域外手段で配布し、短い明示的overlap後に旧credentialを失効する。動的pairing messageは定義しない。
- 公開鍵pinはDiscoveryまたは既存WSS messageだけを根拠に自動更新してはならない（MUST NOT）。新pinを含むreplacement bundleを帯域外で明示的に導入しなければならない（MUST）。
- 計画rotationのため、Workerは管理者が明示的に導入したcurrent pinとnext pinを期限付きで併存させてよい（MAY）。無期限の複数pin許可はすべきではない（SHOULD NOT）。
- 新Sessionは旧Sessionをfenceするが、credential自体をrotationしたことにはならない。
- Worker credentialとDashboard管理者credentialは、形式、保存場所、認可scopeのいずれも共用してはならない（MUST NOT）。

## 5. 共通JSONエンベロープ

WSS上の全protocol messageは、一つのUTF-8 JSON objectとして次のenvelopeを持たなければならない（MUST）。UDPのdiscovery.announceも同じfieldを持つが、session_idだけはnullとする。

~~~json
{
  "protocol_version": "1.0",
  "type": "task.offer",
  "message_id": "0d7dff0d-fd3a-4d3c-8e6f-6f743a97c86a",
  "correlation_id": "0d7dff0d-fd3a-4d3c-8e6f-6f743a97c86a",
  "reply_to_message_id": null,
  "sent_at": "2026-08-18T06:15:00.123Z",
  "session_id": "7d64bb5a-a5c4-493d-8f21-42c25b3a84dd",
  "payload": {}
}
~~~

| Field | Type | 規範 |
|---|---|---|
| protocol_version | MAJOR.MINOR string | negotiation済みversion。session.helloでは希望version、discoveryではadvertise用version |
| type | string | task.offerのようなnamespace形式 |
| message_id | UUID | 論理messageの一意ID。再送で再生成しない |
| correlation_id | UUID | 一連のexchangeを関連付けるID。起点messageでは自身のmessage_idを使用してよい |
| reply_to_message_id | UUID or null | 直接応答するmessage。起点ではnull |
| sent_at | string | UTC RFC 3339。診断専用 |
| session_id | UUID or null | 接続ごとのID。nullはdiscovery.announceだけ |
| payload | object | type固有payload |

次の共通規約を適用する。

- key名と通常のenum値はsnake_caseでなければならない（MUST）。message type、task_type、canonical_terminal_typeのようなnamespaced値はdotで区切り、各segmentをsnake_caseにしなければならない（MUST）。
- message typeはnamespace形式でなければならない（MUST）。
- 時刻はUTC RFC 3339で表現し、送信時はZ表記を使用すべきである（SHOULD）。
- durationは末尾を_ms、容量は末尾を_bytesとする。
- 温度は整数のtemperature_milli_celsiusで表す。浮動小数の摂氏値をWireへ送ってはならない（MUST NOT）。
- JSON integerは-9007199254740991から9007199254740991までのJavaScript safe integer範囲に収めなければならない（MUST）。byte、duration、sequence、countは非負でなければならない（MUST）。
- NaN、Infinity、-Infinity、duplicate object keyを拒否しなければならない（MUST）。JSON numberは有限値だけを許可する。
- 同一MAJOR versionの未知object fieldは無視しなければならない（MUST）。ただし送信側は未知fieldが理解されることへ安全性や正しさを依存させてはならない（MUST NOT）。
- 未知message typeにはunsupported_message_type、未知の必須enum値にはschema_violationの構造化エラーを返さなければならない（MUST）。
- 再送では同じmessage_idと同じsemantic payloadを使わなければならない（MUST）。同じmessage_idで異なるsemantic payloadを送ってはならない（MUST NOT）。

semantic payloadは、JSON解析後のprotocol_version、type、correlation_id、reply_to_message_id、payloadの組である。object key順序と空白は意味を持たないが、array順序、未知field、文字列、数値は意味に含む。sent_atとsession_idはtransport metadataとして比較対象外とする。再接続時は、reconciliationで再送を許可されたmessage、特に未ACK terminal eventに限りsession_idだけを新Sessionへ付け替えられる（MAY）。offered、accepted、start、renewなどの非終端Session-bound messageを新Sessionへ持ち越してはならない（MUST NOT）。送信側は元のsent_atを維持すべきである（SHOULD）。同じmessage_idと異なるsemantic payloadはmessage_id_conflictであり、受信側は状態を変更してはならない（MUST NOT）。

sent_atおよびWorker wall clockは、順序、Lease期限、offer期限、liveness、競合の勝者決定に使用してはならない（MUST NOT）。

本書のmonotonic clockはwall clock調整の影響を受けず、端末のsuspend/sleep中もelapsed timeが進む時計を意味する。実装がそれを保証できない場合、またはclock discontinuityを検知した場合、resume時にactive Leaseをexpired、required thermal sampleをstaleとしてfail-closedに扱わなければならない（MUST）。sleepによってLease、sensor freshness、cooldownを延長してはならない（MUST NOT）。

## 6. IDとSession lifecycle

### 6.1 IDの意味

| ID | 寿命と権威 |
|---|---|
| cluster_id | pairingからcluster廃止まで永続。複数clusterを混同しない |
| orchestrator_id | Orchestrator identityとして永続。process再起動で変えない |
| node_id | 事前pairing時にNodeへ割り当て、daemon再起動やOS再起動で変えない |
| boot_id | Worker daemon起動ごとにWorkerが新規生成する。再接続だけでは変えない |
| session_id | WSS接続ごとにWorkerがUUIDを新規生成し、session.helloのcandidateとして送る |
| task_id | 利用者から見た論理TaskごとにOrchestratorが生成する |
| attempt_id | 割当・実行試行ごとにOrchestratorが生成する。retry時は必ず新規生成する |
| lease_id | 特定Attemptの実行権限ごとにOrchestratorが生成する。別Attemptへ再利用しない |
| message_id | 論理messageごとに送信側が生成し、再送時も変えない |

IDはcredentialではなく、ログへ出る可能性がある。IDだけで認可してはならない（MUST NOT）。

### 6.2 Session確立sequence

~~~mermaid
sequenceDiagram
    participant W as Worker daemon
    participant O as Orchestrator
    W->>O: WSS authentication (Authorization header)
    W->>O: session.hello
    Note over O: identity/version検証<br/>新Sessionをcommitし旧Sessionをfence
    O-->>W: session.welcome
    W->>O: node.describe
    O-->>W: node.describe_ack
    W->>O: initial node.heartbeat
    O-->>W: node.heartbeat_ack
    O-->>W: session.ready
    Note over W,O: session.ready後だけTask割当を開始
~~~

順序は次でなければならない（MUST）。

~~~text
WSS authentication → session.hello → session.welcome → node.describe
→ node.describe_ack → initial node.heartbeat → session.ready → Task割当開始
~~~

Orchestratorは、認証済みnode_id、cluster_id、orchestrator_id、boot_id、version、およびresume情報を検証してからsession.welcomeを永続化しなければならない（MUST）。welcomeのcommitが新Sessionのfencing pointである。旧Sessionから以後届く全messageはstale_sessionとし、Task、Lease、liveness、終端状態を変更してはならない（MUST NOT）。

session.helloは少なくともnode_id、boot_id、supported_protocol_versions、previous_session_id、active_assignment、pending_terminal_message_idsを持つ。active_assignmentがある場合はtask_id、attempt_id、lease_id、lease_revision、engine_state、lease_remaining_msを含める。lease_remaining_msはWorker monotonic clockによる参考値であり、Orchestratorの正本を延長しない。Workerはterminal eventを生成した後もattempt.finalizedを受け取るまでactive_assignmentの照合用metadataを保持し、Engine停止済みならengine_state=stoppedとして申告しなければならない（MUST）。

session.welcomeはselected_protocol_version、Heartbeat/liveness値、message上限、Lease初期値、およびreconciliationを持つ。session.readyはOrchestratorからの最終gateであり、description revisionと受理したinitial heartbeat sequenceを含める。session.readyより前にtask.offerを送ってはならない（MUST NOT）。

stopの場合、WorkerのEngine停止とassignment解消をinitial node.heartbeatで確認するまでsession.readyを送ってはならない（MUST NOT）。quarantineの場合は診断用Sessionを維持してよい（MAY）が、session.readyまたはTaskを送ってはならない（MUST NOT）。continue中に旧ローカルLeaseがready前に失効した場合、WorkerはEngineを停止し、continueを取り消してstop相当として報告しなければならない（MUST）。

### 6.3 Reconciliation

session.welcomeのreconciliation.actionは次のいずれかでなければならない（MUST）。

| Action | 条件 | Workerの動作 |
|---|---|---|
| idle | 両者にactive assignmentがない、Engineは停止済みでpending terminal replayだけがある、またはOrchestrator側の旧Attemptを既に安全に閉じられる | Engineをstopped/readyのままにし、未ACK terminal eventは捨てずにinitial Heartbeat後かつready前に再送する |
| continue | 同一boot_idでEngineが実行中、task_id、attempt_id、lease_id、lease_revisionが正本と完全一致し、両者のLeaseが未失効 | 既存Engineだけを継続する。welcomeをLease更新とみなさず、ready後ただちにlease.renewを行う |
| stop | Workerだけが古い・失効・不明なassignmentを保持する、またはOrchestratorが継続を許可しない | 新規開始せず、実行中ならcooperative stop後に必要なら強制終了し、停止結果を報告する |
| quarantine | 異なるactive Attempt同士の主張、ID衝突、永続化破損など、安全に自動解決できない | Engineを停止し、accepting_tasks=falseのまま管理者介入を待つ |

reconciliation自体はLease更新ではない。continueは旧Leaseの残存権限を新Session上で使う許可にすぎず、期限またはrevisionを延長してはならない（MUST NOT）。旧Sessionはfence済みであり、新Sessionだけがmessageを送れる。

welcomeをcommitするtransactionで、continue対象LeaseはrevisionとOrchestratorのauthoritative deadlineを変えずにnew session_idへrebindしなければならず（MUST）、Workerもlocal deadlineを変更してはならない（MUST NOT）。

pending terminal replayだけを行うidleでは、未知のpending message ID、session.helloのboot_idとLease発行時のboot_id、およびactive_assignmentのtask_id、attempt_id、lease_id、lease_revisionがOrchestratorのlive Leaseと全て一致する場合、OrchestratorはそのLeaseをterminal提出専用でnew Sessionへrebindし、当該eventの採否commitまたはauthoritative deadlineまでrevokeを保留しなければならない（MUST）。このrebindはEngine実行、renewal、新しいoutput生成を許可せず、revisionまたは両者のdeadlineを延長してはならない（MUST NOT）。dedupe recordに既知のmessage IDはLeaseをrebindせず以前と同じattempt.finalizedを返す。一致条件を満たさない未知eventには提出権限をrebindせず、正本に従ってrejected_staleまたはrejected_conflictを返す。

terminal replayがcurrent Attemptを終端へcommitしなかった場合、Orchestratorは当該Leaseをrevokeまたはexpireさせ、Taskを11.2節どおり収束させてからsession.readyを送らなければならない（MUST）。stopまたはquarantineを選ぶ場合、Orchestratorは対応するlive Leaseが存在すればwelcomeと同じtransactionでrevokeしなければならない（MUST）。

Attempt認識不一致は次のように処理する。

- Workerに実行中EngineがなくOrchestratorだけがactiveと認識する場合、上記の同一boot・完全一致するpending terminalがあればterminal-only replayを先に評価する。この例外に該当しない場合、Orchestratorは当該Leaseをrevokeし、Attemptをretry可能な終端へ進めた後、idleまたはstopを選ぶ。Workerへ旧Attemptを再開させてはならない（MUST NOT）。
- Workerだけがactive assignmentを申告し、そのAttemptまたはLeaseが正本に存在しない、expired、revoked、terminalである場合はstopを選ばなければならない（MUST）。
- 両者が異なる非終端Attemptを同一Nodeのactiveとして主張する場合はquarantineを選ばなければならない（MUST）。
- 同じAttemptでもboot_idが変わっている場合、Workerは旧monotonic deadlineを復元できないためcontinueしてはならない（MUST NOT）。stopし、新Attemptを待つ。
- pending terminal eventがある場合、Orchestratorは正本を先に照合する。message IDがdedupe recordにあれば以前と同じattempt.finalizedを返し、未知ならrebind、Attempt、Lease、Cancellation、deadlineを通常どおり検証して採否とattempt.finalizedを新規commitしなければならない（MUST）。
- pending terminal eventがsession.helloで申告された場合、Workerはinitial Heartbeat後に同一message_idで再送し、Orchestratorは全eventへattempt.finalizedを返してからsession.readyを送らなければならない（MUST）。未解決terminal eventを残したまま新Taskを割り当ててはならない（MUST NOT）。

## 7. Node・Healthモデル

Node状態を単一NodeStatus enumへ詰め込んではならない（MUST NOT）。次の軸を独立して扱う。

| 軸 | 内容 | 権威 |
|---|---|---|
| Node identity | node_id、boot_id、display_name、platform概要 | node_idはpairing、その他はWorker報告 |
| Node capabilities | task type、ローカルmodel ID、context/output上限、単一slot | Workerの自己申告。Orchestratorは検証済み事実とみなさない |
| Worker availability | starting、ready、busy、cooling_down、degraded、draining | Workerの現在のadmission状態 |
| Engine state | stopped、starting、loading_model、ready、running、cancelling、faulted | daemonが監督して報告 |
| Safety state | safe、throttled、cooling_down、sensor_fault、emergency_stop | Workerローカル安全機構 |
| Health report | resource、battery、thermal、active IDs、admission | Workerの観測snapshot |
| Orchestrator-derived liveness | online、suspect、offline | Orchestratorのmonotonic受信時刻からのみ導出 |

offlineはWorkerが送る値ではない。availability、engine_state、safety_stateのいずれにもofflineを追加してはならない（MUST NOT）。

node.describeは次の情報を持たなければならない（MUST）。

- identity: node_id、boot_id、display_name、platform
- capabilities: supported_task_types、max_concurrent_inferences（MVPでは1）、models
- modelsの各項目: model_id、context_limit_tokens、max_output_tokens
- safety_policy_summary: policy_id、required sensor kind、設定済み閾値の存在、cooldown/staleness方針。ローカルpathは含めない

model capabilityにファイルpath、download URL、credentialを含めてはならない（MUST NOT）。node.describe_ackはdescription_revisionと、Orchestratorが認識したcapability summaryを返す。

### 7.1 Health report

node.heartbeat.payloadは少なくとも次を含める。

- node_id、boot_id
- heartbeat_sequence
- uptime_ms
- available_memory_bytes
- cpu_usage_percent
- battery_percentageおよびcharging_state
- thermal_readings
- availability、engine_state、safety_state
- current_task_id、current_attempt_id、current_lease_id、current_lease_revision
- accepting_tasks
- admission_reason
- retry_after_ms

charging_stateはcharging、discharging、full、not_charging、unknownのいずれかとする。battery_percentageが取得不能な場合はnullを許可する。admission_reasonはready、busy、cooling_down、degraded、draining、sensor_fault、engine_unavailable、insufficient_memory、shutting_downのいずれかとする。retry_after_msは既知の場合に非負整数、未知または不要な場合にnullとする。

current_task_id、current_attempt_id、current_lease_id、current_lease_revisionはactive assignmentがあるとき全て非null、ないとき全てnullでなければならない（MUST）。一部だけの組合せはinvalid stateとしてdegradedまたはreconciliation対象にする。

heartbeat_sequenceはSessionごとに1から開始し、送信ごとに単調増加しなければならない（MUST）。同じmessage_idの再送では同じsequenceを使う。以前に受理したsequence以下の別messageはhealth snapshotを巻き戻してはならない（MUST NOT）。

## 8. Heartbeatとliveness

HeartbeatはHealth/Livenessのためのmessageであり、Lease更新ではない。node.heartbeatまたはnode.heartbeat_ackの送受信によってLease期限、Lease revision、Task timeoutを変更してはならない（MUST NOT）。Lease更新にはlease.renewとlease.renewedだけを使用する。

Orchestratorは、current Sessionから新しいheartbeat_sequenceのnode.heartbeatを受信したmonotonic時刻をlast_heartbeat_receivedとして記録する。Workerのsent_atとwall clockは診断表示にのみ使用し、liveness判定に使ってはならない（MUST NOT）。同じmessage_idの重複Heartbeatには同じACKを返すが、health snapshotまたはlast_heartbeat_receivedを更新してはならない（MUST NOT）。

Orchestratorは次を導出する。

| Liveness | 初期判定 |
|---|---|
| online | current Sessionがあり、initial Heartbeat受理済みで、last_heartbeat_receivedから15,000 ms未満 |
| suspect | current Session切断直後、または15,000 ms以上30,000 ms未満 |
| offline | 30,000 ms以上、またはSessionがなく猶予を超過 |

交渉可能な初期値はheartbeat_interval_ms=5,000、suspect_after_ms=15,000、offline_after_ms=30,000とする。これは普遍的な固定値ではない。Workerはsession.helloで対応可能範囲を示してよく（MAY）、Orchestratorはsession.welcomeでSessionに使用する値を確定しなければならない（MUST）。suspect_after_msはheartbeat_interval_msより大きく、offline_after_msはsuspect_after_msより大きくなければならない（MUST）。

WebSocket ping/pongはtransport故障検知に使用できる（MAY）が、HeartbeatまたはLease renewalの代替として扱ってはならない（MUST NOT）。

## 9. Task・Attempt・Lease

### 9.1 定義

- Taskは利用者から見た一つの論理的依頼であり、retryしてもtask_idは変わらない。
- Attemptは一回の割当・実行試行であり、再試行ごとにattempt_idを変えなければならない（MUST）。
- Leaseは、特定node_idが特定Attemptを期限内に実行し、その結果を提出する権限である。Leaseは別Node、別Attemptへ移転または再利用してはならない（MUST NOT）。

TaskとAttemptの状態名をWireで表す場合は、以下の小文字snake_case値を使用する。

### 9.2 Task状態遷移

~~~mermaid
stateDiagram-v2
    [*] --> queued
    queued --> dispatching: offerを永続化
    dispatching --> running: attempt.startedを採用
    dispatching --> succeeded: valid successがstartedより先着
    dispatching --> retry_wait: reject / offer timeout / revoke / start失敗
    dispatching --> retry_wait: retryable terminalがstartedより先着
    running --> succeeded: 有効なattempt.succeededをcommit
    running --> retry_wait: retryableなfailed / aborted / expired / revoked
    retry_wait --> queued: retry delay満了
    dispatching --> failed: retry不能
    running --> failed: retry不能またはretry上限
    queued --> cancelled: cancellation commit
    dispatching --> cancelled: cancellation commit
    running --> cancelled: cancellation commitが先勝
    retry_wait --> cancelled: cancellation commit
    succeeded --> [*]
    failed --> [*]
    cancelled --> [*]
~~~

Task stateはqueued、dispatching、running、retry_wait、succeeded、failed、cancelledとする。succeeded、failed、cancelledは終端であり、後着messageで別の終端へ変更してはならない（MUST NOT）。

### 9.3 Attempt状態遷移

~~~mermaid
stateDiagram-v2
    [*] --> offered
    offered --> accepted: task.accept
    offered --> rejected: task.reject / offer timeout
    offered --> revoked: cancellation / dispatch撤回
    accepted --> leased: Leaseをcommitしattempt.start
    accepted --> revoked: start前にrevoke
    leased --> running: attempt.started
    leased --> succeeded: valid successがstartedより先着
    leased --> failed: failureがstartedより先着
    leased --> aborted: abortがstartedより先着
    leased --> cancelling: cancellation commit
    leased --> expired: Lease deadline
    leased --> revoked: attempt.revoke
    running --> succeeded: attempt.succeeded commit
    running --> failed: attempt.failed commit
    running --> aborted: attempt.aborted commit
    running --> expired: Lease deadline
    running --> cancelling: attempt.cancel / local stop
    running --> revoked: attempt.revoke commit
    cancelling --> cancelled: Engine停止後attempt.cancelled
    cancelling --> aborted: safety / lease / forced stop
    rejected --> [*]
    revoked --> [*]
    expired --> [*]
    succeeded --> [*]
    failed --> [*]
    aborted --> [*]
    cancelled --> [*]
~~~

Attempt stateはoffered、accepted、leased、running、succeeded、failed、aborted、cancelling、cancelled、rejected、revoked、expiredとする。一つのAttemptへ異なるterminal eventを二つ採用してはならない（MUST NOT）。

Orchestratorの正本状態とWorkerの物理Engine状態は一時的に異なり得る。例えばrevokeをOrchestratorがcommitした後、Workerがmessageを受信してEngineを停止するまで短時間runningであり得る。正本のfenceはcommit時に発効し、その間の出力は採用しない。

## 10. 二段階ディスパッチ

Task dispatchは必ず次の二段階で行う。

~~~text
Orchestrator → task.offer
Worker       → task.accept / task.reject
Orchestrator → attempt.start（Leaseを発行）
Worker       → assignmentを永続化
Worker       → Engineを開始
Worker       → attempt.started
~~~

- task.offerは候補提示であり、実行許可ではない。Workerはtask.offer受信、検証、task.accept送信だけを理由にモデルloadまたは推論を開始してはならない（MUST NOT）。
- attempt.startだけが新しい推論を開始する権限を与える（MUST）。
- OrchestratorはTask、Attempt、Lease revision 1、およびその期限を永続化してからattempt.startを送らなければならない（MUST）。
- Workerは受理したTask specification、task_id、attempt_id、lease_id、lease_revisionをactive assignmentとして耐再起動ストレージへ永続化してからEngineを開始しなければならない（MUST）。
- active assignmentの永続化に失敗したWorkerはEngineを開始してはならない（MUST NOT）。terminal eventだけはdurably記録できる場合、attempt.abortedのassignment_persistence_failedを永続化して報告しなければならない（MUST）。それも永続化できない場合は成功応答を送らずSessionを閉じ、Engineを停止したままLease expiryに収束させなければならない（MUST）。
- Engine processが実際に開始したことを確認してからattempt.startedを送らなければならない（MUST）。

task.offer.payloadはtask_id、attempt_id、offer_valid_for_ms、および完全なTask specificationを持つ。offer_valid_for_msはWorker受信時からのローカル予約目安であり、Orchestratorがmonotonic clockで管理するoffer期限を延長しない。期限後にOrchestratorへ届いたtask.acceptはstale_attemptとし、Leaseを発行してはならない（MUST NOT）。

task.acceptはtask_id、attempt_id、offer_message_idを持つ。task.rejectは同じIDに加え構造化errorを持つ。acceptは単一slotを一時予約する。`session.welcome`で合意した`reservation_timeout_ms`以内に有効なattempt.startが来なければ、Workerは予約を解放しなければならない（MUST）。予約解放後の遅延startはinvalid_stateとしてEngineを開始してはならない（MUST NOT）。

accepted reservationはacceptを送ったSessionとboot_idにbindする。Sessionがfence/closeされた場合またはboot_idが変わった場合、Workerは予約resourceを解放し、Orchestratorは未leased Attemptをrevokedへ進めなければならない（MUST）。旧task.acceptを新Sessionのattempt.start根拠としてはならず（MUST NOT）、retryには新しいattempt_idと新しいofferを使用する。dedupe recordと以前のtask.accept応答は、予約resource解放後も19節の保持期間中は残す。

attempt.startはtask_id、attempt_id、offer_message_id、accept_message_id、lease_id、lease_revision、lease_duration_ms、renew_after_msを持つ。Workerは現在Sessionで自らacceptしたofferとの完全一致を検証しなければならない（MUST）。

同じmessage_id・同じsemantic payloadのduplicate offerまたはduplicate startには、以前と同じ論理応答を返す。duplicate offerでslot数または予約数を増やしたり、初回受信から測るoffer/reservation deadlineをresetしたりしてはならない（MUST NOT）。duplicate startでassignmentを再作成し、Engineを再起動し、Task timeoutまたはLease deadlineをresetしてはならない（MUST NOT）。同一attempt_idまたはlease_idに異なるstart内容が届いた場合はinvalid_leaseまたはmessage_id_conflictとし、開始してはならない（MUST NOT）。

## 11. Lease更新とfencing

### 11.1 Lease内容

Leaseは少なくとも次を持つ。

| Field | 意味 |
|---|---|
| lease_id | 特定Attemptに一意なUUID |
| lease_revision | 1から始まり更新ごとに単調増加するsafe integer |
| lease_duration_ms | 対応するWorker側anchorから許可を保持できるduration |
| renew_after_ms | 同じanchorからrenewを開始すべき推奨duration |

renew_after_msはlease_duration_msより十分短くなければならない（MUST）。初期例はlease_duration_ms=30,000、renew_after_ms=10,000であるが、session.welcomeで交渉可能とし、protocol固定値として扱わない。

Worker側のanchorは、initial Leaseでは対応するtask.acceptを最初に送ったmonotonic時刻、renewalでは対応するlease.renewを最初に送ったmonotonic時刻とする。Workerはanchor + lease_duration_msをローカルdeadline、anchor + renew_after_msをrenew開始時刻にしなければならない（MUST）。attempt.startまたはlease.renewedの初回到着時に既にこのdeadlineへ達していれば、それを使ってEngineを開始、継続、復活させてはならない（MUST NOT）。遅延attempt.startにはEngineを開始せず、durably記録できる場合はattempt.abortedのlease_expiredを返す。これによりnetwork delayはWorkerの実行可能時間を増やさず、消費する。

Orchestratorは発行をcommitした自身のmonotonic時刻 + lease_duration_msをauthoritative deadlineとして管理する。両者のdeadlineは一致しないため、Worker側deadlineは停止上限を、Orchestrator側deadlineは終端結果の採否を決める。

### 11.2 更新message

- lease.renew（Worker → Orchestrator）はtask_id、attempt_id、lease_id、現在のlease_revisionを持つ。
- lease.renewed（Orchestrator → Worker）は同じID、新しいlease_revision、lease_duration_ms、renew_after_msを持つ。
- lease.denied（Orchestrator → Worker）は同じID、要求revision、および構造化errorを持つ。
- attempt.revoke（Orchestrator → Worker）はtask_id、attempt_id、lease_id、lease_revision、revoke reason、cooperative_stop_after_msを持つ。acceptedだが未leasedの予約を撤回する場合だけlease_idとlease_revisionをnullにでき、Lease発行後は両方を必須とする。

Heartbeat、progress、chunk、attempt.started、TCP/WebSocket activityをLease更新とみなしてはならない（MUST NOT）。

Orchestratorは、current Session、current Attempt、current Lease、期限、非終端Taskをtransaction内で検証し、新revisionと新deadlineを永続化してからlease.renewedを送らなければならない（MUST）。revisionは更新ごとに増加し、逆行または再利用してはならない（MUST NOT）。同じlease.renew messageの再送には、revisionを再度増やさず以前と同じlease.renewedを返さなければならない（MUST）。

Workerが同じlease.renewed messageを重複受信した場合、初回受理時のローカルdeadlineを維持し、重複受信時刻からdeadlineを再計算してはならない（MUST NOT）。

Workerは最高の受理済みrevisionだけを更新要求に使わなければならない（MUST）。古い、未知、飛び越したrevisionはinvalid_lease_revisionとする。renew requestの送信だけでは期限を延長しない。期限内に有効なlease.renewedを受信できなければ、Workerはcooperative cancelと必要な強制終了を前倒しで開始し、ローカルdeadlineまでに新token生成だけでなくEngine processを停止しなければならない（MUST）。local graceをdeadlineより後まで使ってはならない（MUST NOT）。

lease.deniedを受信したWorkerは更新retryで実行延長を試みず、ただちに停止処理を開始し、遅くとも現在のローカルdeadlineまでにEngineを停止しなければならない（MUST）。Workerのsent_atまたはwall clockを根拠に、Orchestratorがexpired Leaseを遡及更新してはならない（MUST NOT）。attempt.revokeやattempt.cancelのcooperative_stop_after_msがWorkerのローカルhard graceより長い場合、Workerはより短いローカル値を使用しなければならない（MUST）。

遅延したlease.renewedがWorkerのローカルdeadline後に届いても、expired Leaseを復活させてはならない（MUST NOT）。Workerは停止状態を維持し、attempt.abortedでlease_expiredを報告する。revoked Leaseも同じlease_idまたはrevisionで復活させてはならない（MUST NOT）。

attempt.succeededおよびattempt.failedは送信時にWorkerが使用したlease_revisionを含める。Orchestratorは、そのrevisionが当該SessionとAttemptへ実際に発行され、そのrevision固有の権限期間内であり、かつLease全体がrevoke、expire、reassignされていない場合だけ計算結果を採用してよい（MAY）。新しいrenewal revisionが存在するだけでは、同じcurrent Sessionの直前revisionをその固有期限より前に不正としない。これはrenewed応答消失を許容するためであり、Session fence、Lease revoke、Attempt変更を越えて古いrevisionを許可するものではない。停止確認eventの扱いは12.1節に従う。

attempt.revokeはOrchestratorでcommitした時点から全revisionを不可逆にfenceする。messageが失われても更新を拒否し、旧結果を採用してはならない（MUST NOT）。Workerはrevoke受信後ただちに停止処理を開始しなければならない（MUST）。

revoke transactionはLease/Attemptだけを変更してTaskをrunningまたはdispatchingに残してはならない（MUST NOT）。Cancellation理由ならTaskをcancelledへ、retryableなSession/resource/credential理由ならretry_waitへ、retry不能なpolicy理由ならfailedへ同じtransactionで進めなければならない（MUST）。新Attemptは旧Leaseのfencing commit後にだけ作成できる。物理的な重複計算を減らすため、OrchestratorはWorkerの停止確認または旧ローカルdeadline相当の経過を待ってから再dispatchすべきである（SHOULD）。

### 11.3 再起動

Worker daemon再起動ではmonotonic基準が失われるため、永続化したLeaseを有効と推定してEngineを再開してはならない（MUST NOT）。新boot_idでassignmentを申告し、reconciliationのstopを完了してから新Attemptを待つ。

daemonは起動時、以前のbootから残存するEngine processまたはIPCを新しいTaskより先に停止・無効化しなければならない（MUST）。停止を確認できない場合はquarantineとし、WSS未接続でもaccepting_tasks=falseを維持しなければならない（MUST）。Engineはdaemon不在のまま推論を継続できないよう監督されなければならない（MUST）。

Orchestrator再起動時は、永続化済みTask、Attempt、Lease、Cancellation、dedupe recordを復元してからdispatchを再開しなければならない（MUST）。再起動前の非終端Leaseは保守的にrevoke/expireとしてfenceし、旧Leaseの結果を新規commitしてはならない（MUST NOT）。別Workerまたは新Attemptへ再割当する前に、対象Workerの停止を確認するか、restart時点から最大lease_duration_msとfencing safety marginが経過するまで待たなければならない（MUST）。

## 12. 完了・失敗・中断・キャンセル

### 12.1 Terminal eventとACK

Workerからのterminal eventは次の四つである。

| Type | 意味 |
|---|---|
| attempt.succeeded | 完全な最終出力を生成した |
| attempt.failed | 推論を完了できない実行上のfailure。retryableを明示する |
| attempt.aborted | Lease失効、安全停止、Engine crash、daemon shutdownなどで中断した |
| attempt.cancelled | cancel要求後、Engineの停止を確認した |

全terminal eventはtask_id、attempt_id、lease_id、lease_revisionを持たなければならない（MUST）。Workerは一Attemptにつきsemantic terminal eventを最大一つだけ生成し、そのmessage_idと完全なpayloadを初回送信前に永続化しなければならない（MUST）。

計算結果を正本へ採用するattempt.succeededとattempt.failedは、Orchestrator commit時にcurrent Attemptと提出権限を持つLeaseの検証を通らなければならない（MUST）。一方、attempt.cancelledと、lease_expired、lease_revoked、session_fencedを理由とするattempt.abortedは、既にLeaseまたはSessionを失効させた後の物理停止確認である。Orchestratorは既知のtask_id、attempt_id、lease_idと停止理由が一致する場合にこれを記録してよい（MAY）が、Leaseを復活させたり、Taskの先行terminal stateを変更したり、計算outputを採用したりしてはならない（MUST NOT）。

assignment_persistence_failed、thermal_pause、critical_temperature、sensor_fault、engine_crashed、forced_termination、daemon_shutdownを理由とするattempt.abortedは、current Node/Attempt/Sessionと未確定のTask、およびその時点までのLease権限を検証できた場合、Attemptのaborted terminalとしてcommitしなければならない（MUST）。既にAttemptがexpired、revoked、cancelled、または別terminalへ確定済みなら、同eventは停止確認としてだけ記録し、先行状態を上書きしてはならない（MUST NOT）。

attempt.finalizedはOrchestratorからWorkerへのdurableなterminal ACKであり、少なくともterminal_message_id、task_id、attempt_id、disposition、canonical_terminal_type、attempt_state、task_stateを持つ。canonical_terminal_typeは正本が採用したprotocol terminal typeとする。Cancellation先勝によりOrchestratorがAttemptをcancelledへ正規化した場合はattempt.cancelledとし、Attemptがexpired/revokedで採用または正規化されたterminal typeがない場合だけnullとする。dispositionはcommitted、rejected_stale、rejected_conflict、cancelled_wonのいずれかとする。拒否時は構造化errorを含める。同じterminal eventの再送には、別のdispositionへ変えず、初回と同じdispositionを返さなければならない（MUST）。

Orchestratorはterminal eventの採否と正本状態、および返すattempt.finalizedを原子的に永続化してからACKを送らなければならない（MUST）。attempt.finalizedが失われた場合、Workerは同じterminal message_idと同じsemantic payloadを再送しなければならない（MUST）。Orchestratorは二重commitせず、以前と同じ論理attempt.finalizedを返さなければならない（MUST）。

Workerはcommittedまたは永続的な不採用を示すattempt.finalizedを受信するまで未ACK terminal eventを削除してはならない（MUST NOT）。新Sessionで再送する場合はenvelopeのsession_idだけをcurrent Sessionへ付け替える。

### 12.2 Cancel

- attempt.cancel（Orchestrator → Worker）はtask_id、attempt_id、lease_id、lease_revision、reason、cooperative_stop_after_msを持つ。
- attempt.cancel_ack（Worker → Orchestrator）はcancel_message_id、現在のengine_state、already_terminalを持つ受信確認である。
- attempt.cancelled（Worker → Orchestrator）はEngine processの停止を確認した後にだけ送るterminal eventである。

attempt.cancel_ackは受信確認だけであり、Engine停止完了、Taskのcancel commit、Lease更新、terminal resultではない。OrchestratorとDashboardはcancel_ackを「停止済み」と表示してはならない（MUST NOT）。

OrchestratorがCancellationを受理するtransactionは、現在状態に応じて次を原子的にcommitしなければならない（MUST）。

- queuedまたはretry_waitではTaskだけをcancelledへ進める。実行Leaseは存在しない。
- offeredまたはacceptedではTaskをcancelled、Attemptをrevokedへ進め、Leaseを発行してはならない（MUST NOT）。Workerがaccepted reservationを保持していれば、lease_id=nullのattempt.revokeで解放を要求する。Engineは未起動なのでattempt.cancel_ackまたはattempt.cancelledを要求しない。
- leasedまたはrunningではTaskをcancelled、Attemptをcancellingへ進め、current Leaseをrevokeする。そのcommit後にattempt.cancelを送る。cancel messageが失われてもrenewal拒否とローカルdeadlineにより実行権限は終了する。
- cancellingへの重複Cancellationは同じcommit結果を返す。既にsucceeded、failed、cancelledのTaskを別terminal stateへ変更してはならない（MUST NOT）。

CompletionとCancellationが競合した場合、Orchestrator上で先にtransaction commitされた方だけを採用しなければならない（MUST）。messageのsent_at、Workerの完了時刻、cancel_ackの到着順を勝者決定に使ってはならない（MUST NOT）。

- attempt.succeededのcommitが先ならTaskはsucceededであり、後のCancellationは既完了としてno-opにする。
- Cancellation commitが先ならTaskはcancelledであり、後のattempt.succeeded、attempt.failed、attempt.abortedは計算terminalとして正本へ採用せず、attempt.finalizedのcancelled_wonを返す。protocol.errorのterminal_conflictをこの競合の代替ACKとして返してはならない（MUST NOT）。
- cancel受信時にEngineが既にterminal eventを永続化済みでも、Workerはcancel_ackを返し、同じterminal eventを送る。最終判断はOrchestratorが行う。

Cancellation commit後にWorkerがattempt.cancelled、attempt.aborted、または既に永続化済みのattempt.succeeded/attempt.failedを送った場合、そのeventはEngineがterminalであることの停止確認として扱う。OrchestratorはTaskをcancelledのまま維持し、Attemptをcancelledへ確定し、計算resultを不採用としてattempt.finalizedのcancelled_wonを返さなければならない（MUST）。Workerが到達不能でterminal eventを得られない場合は、authoritative Lease expiryとfencing safety marginの後にAttemptをcancelledへ確定してよい（MAY）。attempt.cancel_ackだけでこの確定を行ってはならない（MUST NOT）。

安全停止、Lease失効、revokeでは、Workerはcancel commandを待ってはならない（MUST NOT）。cooperative cancel後もローカルhard timeout内に停止しないEngineは強制終了しなければならない（MUST）。

## 13. 推論Taskの型

MVPで許可するtask_typeはinference.chat_completionだけである。Task specificationは少なくとも次を持つ。

| Field | Type | 要件 |
|---|---|---|
| task_type | enum | inference.chat_completionのみ |
| model_id | string | Workerローカルallow-listのopaque ID |
| messages | object array | 1件以上のchat message |
| max_output_tokens | positive safe integer | Worker/model上限以下 |
| temperature | finite number | 0.0以上2.0以下 |
| top_p | finite number | 0.0より大きく1.0以下 |
| seed | safe integer, optional | 指定しても端末・Engine間の再現性は保証しない |
| timeout_ms | positive safe integer | Attempt実行のローカル上限 |

messagesの各要素はroleとcontentだけを必須とし、roleはsystem、user、assistantのいずれか、contentはUTF-8 stringでなければならない（MUST）。tool call、function call、画像、音声、attachment、filesystem path、URL fetch、shell、実行指示を表す専用fieldを定義してはならない（MUST NOT）。

同一MAJORのTask objectに未知fieldがあれば共通規約どおり無視し、そのfieldをshell、file、URL、tool callその他の動作へ結び付けてはならない（MUST NOT）。inference.chat_completion以外のtask_typeはunsupported_taskとして拒否する。prompt本文にURL、path、shell文字列、またはコードが含まれていても、それらはモデルへ渡すplain textにすぎない。Orchestrator、Worker、Engineは、Taskの一部としてfetch、open、execute、importしてはならない（MUST NOT）。

model_idはパスでもURLでもない。MVPではASCII lowercaseのpattern [a-z0-9][a-z0-9._-]{0,127} に制限し、Workerローカルallow-listとの完全一致でのみ解決しなければならない（MUST）。TaskからGGUF path、directory、URL、model repositoryを指定できてはならない（MUST NOT）。モデル転送と自動downloadは行わない。

Workerはtask.offer受信時とEngine開始直前の両方で、次を再検証しなければならない（MUST）。

- task_typeと全fieldの型・範囲
- model_idのallow-list登録とローカル利用可能性
- 選択modelの実際のchat template/tokenizerで得たinput token数とmax_output_tokensの合計がcontext_limit_tokens以下であること
- max_output_tokensとモデル/Engineのoutput上限
- timeout_msと、期限内に完了可能かのadmission判断
- available memoryと単一実行slot
- required sensor、temperature、safety state
- Leaseとcurrent Session

必須field欠落、fieldのWire型・protocol範囲違反、未知roleなどのschema不適合はprotocol.errorのschema_violationとし、Taskとしてadmissionしてはならない（MUST NOT）。schema上は有効だがunsupported task_type、model allow-list、context/output limit、memory、deadline、Engine、安全policyに適合しない場合はtask.rejectのstructured codeを使用する。Lease発行後にtokenizer、model load、Engine応答などで判明した不適合はattempt.failedまたはattempt.abortedで報告する。Task timeoutはEngine開始を確認したWorker monotonic時刻から測定し、model load、tokenize、prefill、generation、finalizationを含む。Lease renewalによってTask timeoutをresetまたは延長してはならない（MUST NOT）。

Task timeout、Lease local deadline、安全停止のうち最初に成立した制約を優先しなければならない（MUST）。一方の延長または回復で、既に成立した別の停止条件を取り消してはならない（MUST NOT）。

Engineがreasoning、analysis、scratchpadなどの別channelを返しても、daemonはそれをprotocol message、永続terminal event、ログ、CUI、Dashboardへ出してはならない（MUST NOT）。外部向けassistant contentだけをresult候補にできる。

## 14. ストリーミング

Workerは表示用のbest-effort eventとしてattempt.progressとattempt.output_chunkを送ってよい（MAY）。両messageは同じAttempt内で共有する1始まりのsequenceを持ち、event生成ごとに単調増加しなければならない（MUST）。continue reconciliation後もsequenceをresetしてはならない（MUST NOT）。

attempt.progress.payloadはtask_id、attempt_id、lease_id、lease_revision、sequence、stage、elapsed_msを持つ。stageはpreparing、loading_model、prefill、generating、finalizing、cancellingに限定する。input_tokens、output_tokens、tokens_per_second_milliなどの外部向けmetricsは含めてよい（MAY）。

attempt.output_chunk.payloadはtask_id、attempt_id、lease_id、lease_revision、sequence、text、output_tokens_totalを持つ。textは外部向けassistant outputのdeltaだけであり、内部思考を含めてはならない（MUST NOT）。

- progress/chunkはbest effortであり、永続化と再送を要求しない。欠落、重複、順序の入れ替わりを許容する。
- 受信側はsequenceで重複を除外し、欠番を待ってTask完了を阻害してはならない（MUST NOT）。
- 同じAttemptで同じsequenceに異なる内容を割り当ててはならない（MUST NOT）。
- stale Session、stale Attempt、無効Leaseのchunkを正本表示、利用者表示、確定resultへ採用してはならない（MUST NOT）。
- chunkの連結結果を確定結果として使用してはならない（MUST NOT）。
- attempt.succeededはchunk受信状況に依存せず、完全な最終出力を含めなければならない（MUST）。
- backpressure時はprogressを集約し、chunkを破棄してよい（MAY）。terminal result、Lease、Cancel、その他control messageを同じ理由で黙って破棄してはならない（MUST NOT）。
- progress/chunkはHeartbeatでもLease renewalでもない。
- hidden chain-of-thought、reasoning token本文、logit、scratchpadを運ぶfieldを追加してはならない（MUST NOT）。

attempt.succeeded.payloadは少なくともtask_id、attempt_id、lease_id、lease_revision、result、usageを持つ。resultはrole=assistantの完全なmessageとfinish_reasonを持ち、finish_reasonはMVPではstopまたはmax_output_tokensとする。usageはinput_tokens、output_tokens、total_tokensを持つ。last_sequenceを送る場合、それはWorkerが生成した最後のstream sequenceであり、受信済みchunkの完全性を意味しない。durationおよび速度metricsを追加してよい（MAY）。

## 15. 温度・安全制御

### 15.1 Thermal reading

thermal_readingsの各要素は次を持つ。

| Field | Type | 規範 |
|---|---|---|
| sensor_id | string | Node内で一意なopaque ID |
| kind | enum | battery、soc、cpu、skin、other |
| source | enum | android_thermal_api、termux_api、sysfs、other |
| status | enum | valid、unavailable、stale |
| temperature_milli_celsius | integer | validとstaleで必須。unavailableで値がなければ省略 |
| sampled_at_uptime_ms | safe integer | 最終sampleのWorker uptime。未取得なら省略 |

status=staleのtemperature_milli_celsiusは最後に得た値であって現在値ではない。stalenessはsent_atやwall clockではなく、Workerのmonotonic uptimeで判定しなければならない（MUST）。

Workerのthermal報告はOrchestratorから見たattestationではない。改変Workerは虚偽を報告できる。一方、正しく動作するWorker daemonはPython Engineから独立してsensorを監視し、そのローカル判断をOrchestrator接続の有無にかかわらず実施しなければならない（MUST）。

### 15.2 ローカル安全policy

Workerのローカルpolicyは少なくとも次を定義しなければならない（MUST）。

- required sensor kindと必要count
- sensor kindごとのpause_threshold_milli_celsius
- pause thresholdより低いresume_threshold_milli_celsius
- critical_threshold_milli_celsius
- minimum_cooldown_ms
- sensor_staleness_deadline_ms
- cooperative cancel後にEngineを強制終了するlocal grace

required sensor kindごとに、freshかつvalidなsensor数が設定count未満ならsensor faultとする。同じkindの複数sensorが有効な場合、pause/critical判定には最も高いtemperatureを使用しなければならない（MUST）。optional sensorのcritical超過も、そのsensorにruleが設定されていれば無視してはならない（MUST NOT）。

各sensor ruleは次の順序を満たさなければならない（MUST）。

~~~text
resume_threshold_milli_celsius
  < pause_threshold_milli_celsius
  < critical_threshold_milli_celsius
~~~

閾値は端末、sensor種別、筐体、電源状態によって異なるため、40℃その他の全端末共通値をWire protocolへ固定しない。node.describeで有効policy summaryを報告してよい（MAY）が、自己申告であり、remote mutation interfaceではない。

安全状態と必須動作は次のとおりである。

- pause未満で性能抑制だけを行う場合はthrottledとする。
- required sensorのいずれかがpause threshold以上になった場合、accepting_tasks=false、availability=cooling_down、safety_state=cooling_downとし、実行中Attemptをローカルに中断しなければならない（MUST）。
- required sensorの不足、unavailable、またはstaleness deadline超過はfail-closedとし、safety_state=sensor_fault、accepting_tasks=falseにし、実行中Attemptを中断しなければならない（MUST）。optional sensorだけの故障はpolicyに従いdegradedとしてよい（MAY）。
- critical threshold到達時はsafety_state=emergency_stopとし、ただちにcooperative cancelを開始し、local grace内に止まらなければEngineを強制終了しなければならない（MUST）。
- pauseまたはsensor faultによる中断はattempt.abortedでthermal_pauseまたはsensor_faultを報告する。critical時はcritical_temperatureを使用する。
- 再開には、全required sensorがfreshかつresume threshold以下の状態をminimum_cooldown_ms連続して満たさなければならない（MUST）。これがhysteresisであり、一時的な温度低下だけで再開してはならない（MUST NOT）。
- emergency_stopはMVPではlatchし、安全条件回復後もWorkerローカルのpolicyが定める明示的resetなしに解除してはならない（MUST NOT）。Orchestratorから解除できてはならない（MUST NOT）。
- Orchestrator切断中もsensor監視、Lease watchdog、cooperative cancel、強制終了、cooldownを動作させなければならない（MUST）。

MVPでは安全閾値、required sensor、staleness、cooldown、force-kill graceを変更するWire messageを定義しない。OrchestratorはTaskを割り当てない、cancelするなど、より厳しい判断を行える（MAY）が、Workerのhard limitを緩和したり、安全装置を無効化したりしてはならない（MUST NOT）。

## 16. Engine境界

Worker daemonだけがWSSとNode credentialを扱い、Python EngineをLANへ公開してはならない（MUST NOT）。

- daemonとEngine間IPCはUnix domain socketを第一候補とすべきである（SHOULD）。
- Unix domain socketとそのparent directoryはdaemon/Engine identity以外から接続・置換できないaccess controlを持たなければならない（MUST）。既存socket、symlink、別ownerのendpointを安全確認なしに再利用してはならない（MUST NOT）。
- TCP fallbackを使う場合は正確に127.0.0.1へだけbindしなければならない（MUST）。IPv6を使う場合は[::1]だけを許可する。0.0.0.0、[::]、LAN interfaceへbindしてはならない（MUST NOT）。
- loopback TCPではdaemon起動ごとに生成する256-bit以上のrandom tokenを使用しなければならない（MUST）。tokenをTask、argv、protocol message、error、ログへ出してはならない（MUST NOT）。
- FastAPIその他のEngine HTTP APIをLANへ公開してはならない（MUST NOT）。
- daemonはEngine process、単一実行slot、model load、timeout、crash、cancel、force kill、restart backoffを監督しなければならない（MUST）。
- Engine crash/restart後にactive Attemptを自動再開してはならない（MUST NOT）。正本とのreconciliationまたは新しいattempt.startを待つ。
- Task値は型付きIPC fieldとして渡し、shell fragment、command line、environment expansion、module名として利用してはならない（MUST NOT）。
- model_idからpathを直接組み立ててはならない（MUST NOT）。daemon管理のallow-listだけがmodel_idをローカルGGUF pathへ解決できる。
- Engine応答はuntrusted dataとし、daemonがUTF-8、型、size、token count、Attempt/Lease対応を検証しなければならない（MUST）。
- protocol errorのdetailへstack trace、ローカルpath、environment、credential、prompt/result断片を出してはならない（MUST NOT）。

Worker CUIはモデル出力、Node表示名、error detailをuntrusted plain textとして扱い、Rich markupとANSI/C0/C1制御文字を表示前にescapeまたは可視化しなければならない（MUST）。DashboardのVue実装は通常のtext interpolationまたはtext nodeを使用し、untrusted dataをv-htmlその他のraw HTML sinkへ直接渡してはならない（MUST NOT）。

Engineが生成したコード、command、URLは出力textとして表示できるが、自動実行または自動取得してはならない（MUST NOT）。Engine由来のhidden reasoning channelはdaemon境界で破棄しなければならない（MUST）。

## 17. Message registry

方向のOはOrchestrator、WはWorker daemonを表す。discovery.announce以外は認証済みWSS text messageである。

| Message type | 方向 | 用途 |
|---|---|---|
| discovery.announce | O → LAN / UDP | WSS接続先の発見。認証・Task配信ではない |
| session.hello | W → O | Node、boot、対応version、resume状態を提示 |
| session.welcome | O → W | version、Session parameter、reconciliationを確定 |
| session.reject | O → W | helloのidentity、version、状態不一致を通知して切断 |
| session.ready | O → W | describe、initial heartbeat、reconciliation後にTask割当を解禁 |
| node.describe | W → O | identity、capabilities、model ID、安全policy概要を報告 |
| node.describe_ack | O → W | descriptionの受理とrevisionを応答 |
| node.heartbeat | W → O | health、availability、Engine、安全、active IDsを報告 |
| node.heartbeat_ack | O → W | Heartbeat受信確認。Leaseは更新しない |
| task.offer | O → W | 実行候補を提示。実行権限ではない |
| task.accept | W → O | Offerを検証し単一slotへ予約 |
| task.reject | W → O | Offerを構造化理由で拒否 |
| attempt.start | O → W | 永続化済みLeaseにより新規実行を許可 |
| attempt.started | W → O | assignment永続化とEngine開始を報告 |
| attempt.progress | W → O | best-effortの処理段階・metrics |
| attempt.output_chunk | W → O | best-effortの表示用text delta |
| attempt.succeeded | W → O | 完全な最終出力を持つterminal event |
| attempt.failed | W → O | 推論・Engineの構造化failure |
| attempt.aborted | W → O | Lease、安全、crash等による中断 |
| attempt.finalized | O → W | terminal eventを永続処理したdurable ACK |
| attempt.cancel | O → W | Cancellationと停止要求 |
| attempt.cancel_ack | W → O | cancel受信確認。停止完了ではない |
| attempt.cancelled | W → O | Engine停止確認後のterminal event |
| attempt.revoke | O → W | Leaseを不可逆にfenceして停止を要求、または未leased予約を撤回 |
| lease.renew | W → O | current Lease revisionの明示的更新要求 |
| lease.renewed | O → W | 永続化済み新revisionとduration |
| lease.denied | O → W | Lease更新拒否 |
| protocol.error | 双方向 | 構造化protocol violation。Task resultではない |

attempt.startedはtask_id、attempt_id、lease_id、lease_revision、started_at_uptime_msを持つ。attempt.startedが消失または遅延しても、有効なLeaseを伴うterminal eventが先に届いた場合、Orchestratorはleasedから対応terminal stateへ直接commitしてよい（MAY）。startedの欠落だけを理由に完全な有効resultを捨てるべきではない（SHOULD NOT）。

node.heartbeat_ackはheartbeat_sequenceとOrchestratorの診断用received_atだけを持ち、Lease fieldを持ってはならない（MUST NOT）。session.reject、task.reject、lease.denied、protocol.errorは次節の共通error objectを使用する。

## 18. エラーコード

制御判断はhuman-readable messageではなく、次のstructured error objectで行わなければならない（MUST）。

~~~json
{
  "code": "cooling_down",
  "retryable": true,
  "retry_after_ms": 30000,
  "detail": "optional bounded diagnostic text"
}
~~~

codeとretryableは必須である。retry_after_msはretryable=trueかつ見積可能な場合にだけ付ける。detailは任意の表示専用textであり、受信側は制御判断のためにparseしてはならない（MUST NOT）。detailはsize制限を受け、credential、Authorization、prompt、result、stack trace、ローカルpath、environmentを含めてはならない（MUST NOT）。

### 18.1 Task rejection code

| Code | 意味 |
|---|---|
| busy | 単一slotが使用中または予約済み |
| cooling_down | hysteresis/cooldown未完了 |
| sensor_fault | required sensorが不足、unavailable、stale |
| insufficient_memory | 安全にmodel loadまたは推論できない |
| model_unavailable | model_idがallow-list外またはローカル利用不能 |
| unsupported_task | inference.chat_completion以外 |
| engine_unavailable | Engineがstopped、faulted、restart backoff中 |
| deadline_unreachable | timeout/offer期限内の開始・完了が見込めない |
| policy_denied | ローカル安全/resource policyで拒否 |
| shutting_down | drainingまたはdaemon終了中 |

retryableはcodeから暗黙推測してはならない（MUST NOT）。同じcodeでも別Nodeへの即時retryと同一Nodeへのretry可能時刻が異なり得るためである。

### 18.2 Protocol error code

| Code | 意味 |
|---|---|
| malformed_json | syntax error、invalid UTF-8、NaN/Infinity相当、duplicate key |
| schema_violation | 必須field、型、範囲、未知の必須enum値が不正 |
| unsupported_version | 共通versionがない、または選択version違反 |
| unsupported_message_type | negotiated versionで未知のtype |
| invalid_state | 現在の状態遷移で許可されないmessage |
| stale_session | fence済みSessionからのmessage |
| stale_attempt | currentでない、またはoffer期限切れAttempt |
| invalid_lease | Lease ID不一致、expired、revoked、別Attempt |
| invalid_lease_revision | revisionの逆行、不一致、未発行値 |
| terminal_conflict | 別terminal eventまたはCancellationが先にcommit済み |
| rate_limited | Sessionのmessage rate上限超過 |
| message_id_conflict | 同一message IDに異なるsemantic payload |
| payload_too_large | type別または全体size上限超過 |
| binary_frame | JSON text以外のWebSocket message |
| sequence_conflict | 同じAttempt sequenceに異なるstream event |

session.reject、task.reject、lease.denied、attempt.failed、attempt.abortedはpayload.errorにこの共通objectを入れる。protocol.error.payloadはerror、related_message_id（判明している場合）、fatalを持つ。malformedでmessage IDを得られない場合はrelated_message_idをnullにできる。errorを安全に送れないmalformed、oversized、binary、認証前failureでは、受信側はWebSocket closeまたはHTTP拒否だけを行ってよい（MAY）。

stale_session、message_id_conflict、継続的rate_limitedはfatalとしてSessionを閉じてよい（MAY）。protocol.errorへさらにprotocol.errorを返して再帰させてはならない（MUST NOT）。

session.rejectでは少なくともunsupported_version、identity_mismatch、credential_revoked、invalid_resume_stateを使用できる。認証失敗をWebSocket upgrade前に処理できる場合は、session.rejectを送らず一般化したHTTP errorとする。

attempt.failedのcodeはcontext_limit_exceeded、model_load_failed、generation_timeout、inference_error、invalid_engine_response、output_too_largeを使用できる。attempt.abortedのcodeはassignment_persistence_failed、lease_expired、lease_revoked、thermal_pause、critical_temperature、sensor_fault、engine_crashed、forced_termination、daemon_shutdown、session_fencedを使用できる。これらもretryableを明示しなければならない（MUST）。

## 19. Idempotencyと永続化

### 19.1 重複排除

Orchestratorは(authenticated node_id, message_id)を、Workerは(paired orchestrator_id, message_id)を重複排除keyとして使用しなければならない（MUST）。payload内の自己申告node_idをauthenticated node_idの代用としてはならない（MUST NOT）。

受信処理順序は次でなければならない（MUST）。

1. transportと認証を検証する。session.helloでは未確立のcandidate session_idとprevious_session_idを検証し、welcome commit後の全messageではcurrent session_idとの一致を検証する。
2. stale Sessionならstale_sessionとして拒否し、Task系dedupe recordへ処理済みとして登録しない。
3. message IDの既存semantic payloadを照合する。
4. state、Attempt、Lease、schemaを検証する。
5. 必要な正本変更と応答を永続化する。
6. 応答を送る。

current Sessionで同じdedupe keyと同じsemantic payloadを受信した場合、受信側は処理を再実行せず、以前に永続化したものと同じ論理応答を返さなければならない（MUST）。論理応答にはresponse message_id、type、correlation、payloadを含む。reconciliation後の再送では応答envelopeのsession_idだけをcurrent Sessionへ付け替えてよい（MAY）。fence済み旧Sessionからの同一messageは、この規則より先にstale_sessionとして拒否する。

同じmessage IDで異なるsemantic payloadを受信した場合はmessage_id_conflictであり、状態を変更してはならない（MUST NOT）。object key順と空白だけの違いはpayload差ではない。

- duplicate task.offerで予約slot、busy count、Attempt数を増やしてはならない（MUST NOT）。
- duplicate attempt.startでassignmentの再作成、Engine再起動、二つ目のprocess生成をしてはならない（MUST NOT）。
- duplicate lease.renewでrevisionを二度増やしてはならない（MUST NOT）。
- 同じterminal message_idの再送は同じattempt.finalizedを返し、terminal commitを増やしてはならない（MUST NOT）。
- 同一Attemptへ別のterminal message IDが届いた場合、typeやpayloadが同一でも二つ目はterminal_conflictとして先のcommitを維持しなければならない（MUST）。

### 19.2 永続化境界

Orchestratorは次を、対応するACK、attempt.start、lease.renewed、attempt.finalizedを送信する前に、原子的かつ再起動後も残る形で永続化しなければならない（MUST）。

- Task状態とその更新順序
- Attempt状態、割当Node、採用済みterminal event
- Lease ID、revision、duration、authoritative deadline、revoke/expire
- Cancellation intent、commit順序、勝者
- terminal result本文と採否
- Task関連messageのdedupe recordと返す論理応答

Workerは次を永続化しなければならない（MUST）。

- accepted offerの単一slot予約（予約解放まで）と、そのmessageのdedupe応答（19.2節の保持期間まで）
- attempt.startで受け取ったactive assignment。Task specification、task_id、attempt_id、lease_id、lease_revision、実行状態を含め、Engine開始前にcommitする
- cancel intent。attempt.cancel_ackを返す前にcommitする
- 未ACK terminal eventのmessage ID、semantic payload、元のsent_at。初回送信前にcommitする

永続化に失敗した側は、未永続の状態変更を成功としてACKしてはならない（MUST NOT）。Workerはactive assignmentを永続化できなければEngineを開始してはならない（MUST NOT）。

Workerはterminal eventを生成してEngineが停止した後も、attempt.finalizedを受け取るまでactive assignmentの照合用metadataと単一slotを保持しなければならない（MUST）。これを新しい推論の実行権限として扱ってはならない（MUST NOT）。

Workerはattempt.finalizedによりterminal eventのcommitまたは永続的な不採用を確認するまで、そのeventを削除してはならない（MUST NOT）。reconnect後はcurrent session_idを付け、同じmessage IDとsemantic payloadで再送しなければならない（MUST）。

通常control messageのdedupe recordは少なくとも再接続猶予期間まで保持しなければならない（MUST）。Task、Attempt、Lease、Cancellationのrecordは対応Taskの保持期間中、かつpeerが正当に再送し得る間は保持しなければならない（MUST）。active Taskまたは未ACK terminal eventに必要なrecordをgarbage collectしてはならない（MUST NOT）。

attempt.finalizedにはACK-of-ACKを設けないため、MVPのOrchestratorは一度受信したterminal eventについて、authenticated node_id、message_id、衝突耐性のあるsemantic payload digestと完全比較に必要な情報、および同じ論理attempt.finalizedを再構成できるcompact tombstoneをcluster lifetime中保持しなければならない（MUST）。digest一致だけで異なるpayloadを同一と判定してはならない（MUST NOT）。これにより長期offline後の再送でも二重commitと応答変化を防ぐ。full prompt/resultの保持期間を同じ長さにする必要はない。MVPはstorage製品、DB、file形式を規定せず、原子性、耐再起動性、順序性だけを規定する。

## 20. Versioningと言語横断契約

Wire protocol versionはアプリケーション、Rust crate、Python package、Web UIのversionから分離し、MAJOR.MINOR形式とする。MVPは1.0である。

session.hello.payloadはsupported_protocol_versionsを重複のないversion集合として降順で送り、envelopeのprotocol_versionには先頭の最高versionを設定しなければならない（MUST）。Orchestratorは完全一致するversionのうち最も高いものをsession.welcome.payload.selected_protocol_versionで選ばなければならない（MUST）。共通versionがなければsession.rejectのunsupported_versionを返し、Task割当を開始してはならない（MUST NOT）。

session.helloとsession.rejectはbootstrap messageとして、対応をadvertiseした各MAJORの最低限envelopeをparseできなければならない（MUST）。welcome後の全WSS messageはselected_protocol_versionを設定しなければならない（MUST）。protocol_versionはmessageのsemantic identityに含まれるため、未ACK terminal eventがあるWorkerは、そのeventを生成したものと完全に同じversionを選べるようsupported_protocol_versionsを制限しなければならない（MUST）。異なるminorへ書き換えて同じmessage IDを再送してはならない（MUST NOT）。完全一致versionを選べなければterminal replayを行わずquarantineとする。

次の変更はMAJOR更新を必要とする（MUST）。

- fieldの削除
- fieldの型、単位、nullable性の非互換変更
- 既存enum値または状態遷移の意味変更
- 既存messageの実行権限、安全性、idempotency、fencing semanticsの変更

MINOR更新ではoptional object field、既存処理に影響しないmessage type、または明示的にfeature negotiationされたmessageを追加してよい（MAY）。同一MAJORの未知object fieldは無視する一方、送信側はnegotiationされていないfieldへ正しさを依存させてはならない（MUST NOT）。未知message typeと未知の必須enum値は黙って無視してはならない（MUST NOT）。

本書をprotocol semanticsの正本とする。将来作成するJSON Schemaと固定JSON fixturesをWire shapeの正本とする。Rustの型、Pythonのmodel、TypeScriptの型のいずれか一つだけを正本としてはならない（MUST NOT）。各言語実装は同じ正常fixtureと異常fixtureをCIで検証しなければならない（MUST）。

将来の配置例は次とする。本書の作成範囲にはSchema本体、fixture本体、CI設定を含めない。

~~~text
schemas/protocol/v1/
├── envelope.schema.json
├── messages.schema.json
└── fixtures/
~~~

## 21. 制限値とDoS対策

次はMVPの交渉可能な初期上限案である。Discoveryと交渉前session.helloにはpre-session上限を適用する。それ以外はsession.welcomeで選択した値に送信側が従わなければならない（MUST）。受信側は自身のhard limitを越える値へ合意してはならず（MUST NOT）、より小さい値を選んでよい（MAY）。

| 対象 | 初期上限 | 測定対象 |
|---|---:|---|
| discovery packet | 1,200 bytes | UDP payload全体 |
| WebSocket message | 2,097,152 bytes | fragment再構成後、展開後のUTF-8 bytes |
| session.hello | 16,384 bytes | envelope全体 |
| node.describe / capabilities | 262,144 bytes | envelope全体 |
| node.heartbeat | 65,536 bytes | envelope全体 |
| prompt | 524,288 bytes | messagesのUTF-8 JSON表現 |
| final result | 1,048,576 bytes | terminal message全体 |
| stream chunk | 16,384 bytes | textのUTF-8 bytes |
| error detail | 4,096 bytes | detailのUTF-8 bytes |
| thermal sensor count | 32 | 一report内 |
| model count | 128 | 一node.describe内 |
| chat message count | 128 | 一Task内 |
| JSON nesting | 32 levels | top-level objectをlevel 1 |
| message rate | sustained 50/second、burst 100 | authenticated Session・受信方向ごとのtoken bucket |

これらは初期値であり、将来のminor versionやsession.welcomeで安全に小さくできる。messageごとの上限が全体WebSocket上限より優先する。

- WSSではUTF-8 JSON text messageだけを許可し、一つのWebSocket messageに一つのJSON objectだけを含めなければならない（MUST）。
- binary message、JSON array batch、複数objectの連結を拒否しなければならない（MUST）。
- oversized messageは可能な限りJSON全体をmemoryへ構築する前に拒否しなければならない（MUST）。安全にerrorを返せる場合はpayload_too_largeを返し、WebSocket close code 1009相当で閉じてよい（MAY）。
- permessage-deflateは無効にすべきである（SHOULD）。有効にする場合、圧縮前だけでなく展開後bytesにも同じ上限を適用しなければならない（MUST）。
- malformed、過剰nesting、duplicate keyをbounded resourceで拒否しなければならない（MUST）。
- message rate超過にはrate_limitedとretry_after_msを返し、継続的違反ではSessionを閉じてよい（MAY）。
- backpressure時にattempt.progressを集約し、attempt.output_chunkを破棄してよい（MAY）。
- 完全なfinal resultが上限を超える場合、Workerはresultを黙ってtruncateまたはchunkだけで代用してはならず（MUST NOT）、attempt.failedのoutput_too_largeを送らなければならない（MUST）。
- Task control、Session control、Heartbeat、Lease、Cancellation、terminal resultを黙って破棄してはならない（MUST NOT）。処理できない場合は構造化errorを返すか接続を閉じ、永続化済みmessageの再送を可能にしなければならない（MUST）。
- 送信queueはLease、Cancellation、terminal resultなどのcontrol trafficがstream chunkにstarveされないよう優先制御すべきである（SHOULD）。

## 22. 代表JSON例

以下の例では同一の正常系を通してIDを一貫させる。必須例に含まれないnode.describe、node.describe_ack、session.ready、attempt.startedなどの中間messageは規定順に発生済みとして例示を省略する。task.rejectだけは、primary Attemptの実行中に別Taskがofferされた代替分岐であり、primary task.acceptと矛盾しない。credentialはJSONへ出してはならないため、例にも含めない。

| 種別 | 値 |
|---|---|
| cluster_id | 11111111-1111-4111-8111-111111111111 |
| orchestrator_id | 22222222-2222-4222-8222-222222222222 |
| node_id | 33333333-3333-4333-8333-333333333333 |
| boot_id | 44444444-4444-4444-8444-444444444444 |
| session_id | 55555555-5555-4555-8555-555555555555 |
| primary task_id | 66666666-6666-4666-8666-666666666666 |
| primary attempt_id | 77777777-7777-4777-8777-777777777777 |
| primary lease_id | 88888888-8888-4888-8888-888888888888 |
| rejected task_id | 99999999-9999-4999-8999-999999999999 |
| rejected attempt_id | aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa |

### 22.1 discovery.announce

discovery.announceはSession確立前のためsession_id=nullである。UDP packetにはこのJSON objectだけを載せる。

~~~json
{
  "protocol_version": "1.0",
  "type": "discovery.announce",
  "message_id": "b0000001-0000-4000-8000-000000000001",
  "correlation_id": "b0000001-0000-4000-8000-000000000001",
  "reply_to_message_id": null,
  "sent_at": "2026-08-18T06:14:55.000Z",
  "session_id": null,
  "payload": {
    "discovery_version": "1.0",
    "cluster_id": "11111111-1111-4111-8111-111111111111",
    "orchestrator_id": "22222222-2222-4222-8222-222222222222",
    "worker_wss_url": "wss://orchestrator.pocketswarm.lan:7443/worker",
    "supported_protocol_versions": [
      "1.0"
    ],
    "orchestrator_public_key_sha256_pin": "sha256/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "valid_for_ms": 10000,
    "nonce": "UFNXX2Rpc2NvdmVyeV9ub25jZV8wMQ"
  }
}
~~~

### 22.2 session.hello

session_idはWorkerがこの接続用に生成したcandidateである。Authorization credentialはWSS upgrade headerだけに存在する。

~~~json
{
  "protocol_version": "1.0",
  "type": "session.hello",
  "message_id": "b0000002-0000-4000-8000-000000000002",
  "correlation_id": "b0000002-0000-4000-8000-000000000002",
  "reply_to_message_id": null,
  "sent_at": "2026-08-18T06:15:00.000Z",
  "session_id": "55555555-5555-4555-8555-555555555555",
  "payload": {
    "cluster_id": "11111111-1111-4111-8111-111111111111",
    "orchestrator_id": "22222222-2222-4222-8222-222222222222",
    "node_id": "33333333-3333-4333-8333-333333333333",
    "boot_id": "44444444-4444-4444-8444-444444444444",
    "supported_protocol_versions": [
      "1.0"
    ],
    "previous_session_id": null,
    "active_assignment": null,
    "pending_terminal_message_ids": [],
    "heartbeat_preferences": {
      "minimum_interval_ms": 1000,
      "maximum_interval_ms": 30000
    }
  }
}
~~~

### 22.3 session.welcome

初期接続なのでreconciliation.actionはidleである。以下のtimeoutとlimitはこのSessionで選択された値であり、protocol共通の不変値ではない。

~~~json
{
  "protocol_version": "1.0",
  "type": "session.welcome",
  "message_id": "b0000003-0000-4000-8000-000000000003",
  "correlation_id": "b0000002-0000-4000-8000-000000000002",
  "reply_to_message_id": "b0000002-0000-4000-8000-000000000002",
  "sent_at": "2026-08-18T06:15:00.050Z",
  "session_id": "55555555-5555-4555-8555-555555555555",
  "payload": {
    "cluster_id": "11111111-1111-4111-8111-111111111111",
    "orchestrator_id": "22222222-2222-4222-8222-222222222222",
    "node_id": "33333333-3333-4333-8333-333333333333",
    "selected_protocol_version": "1.0",
    "heartbeat_interval_ms": 5000,
    "suspect_after_ms": 15000,
    "offline_after_ms": 30000,
    "offer_timeout_ms": 10000,
    "reservation_timeout_ms": 15000,
    "lease_defaults": {
      "lease_duration_ms": 30000,
      "renew_after_ms": 10000,
      "maximum_lease_duration_ms": 30000
    },
    "limits": {
      "max_websocket_message_bytes": 2097152,
      "max_node_describe_bytes": 262144,
      "max_heartbeat_bytes": 65536,
      "max_prompt_bytes": 524288,
      "max_final_result_bytes": 1048576,
      "max_stream_chunk_bytes": 16384,
      "max_error_detail_bytes": 4096,
      "max_thermal_sensor_count": 32,
      "max_model_count": 128,
      "max_chat_message_count": 128,
      "max_json_nesting": 32,
      "max_message_rate_per_second": 50,
      "message_rate_burst": 100
    },
    "reconciliation": {
      "action": "idle",
      "reason": "state_match",
      "task_id": null,
      "attempt_id": null,
      "lease_id": null,
      "lease_revision": null
    }
  }
}
~~~

### 22.4 node.heartbeat

これはsession.ready前のinitial Heartbeatである。current assignmentはなく、Lease durationや更新fieldもない。offlineはWorker payloadに存在しない。

~~~json
{
  "protocol_version": "1.0",
  "type": "node.heartbeat",
  "message_id": "b0000004-0000-4000-8000-000000000004",
  "correlation_id": "b0000004-0000-4000-8000-000000000004",
  "reply_to_message_id": null,
  "sent_at": "2026-08-18T06:15:05.000Z",
  "session_id": "55555555-5555-4555-8555-555555555555",
  "payload": {
    "node_id": "33333333-3333-4333-8333-333333333333",
    "boot_id": "44444444-4444-4444-8444-444444444444",
    "heartbeat_sequence": 1,
    "uptime_ms": 15000,
    "available_memory_bytes": 3221225472,
    "cpu_usage_percent": 18.25,
    "battery_percentage": 78,
    "charging_state": "discharging",
    "thermal_readings": [
      {
        "sensor_id": "battery_0",
        "kind": "battery",
        "source": "android_thermal_api",
        "status": "valid",
        "temperature_milli_celsius": 36500,
        "sampled_at_uptime_ms": 14980
      },
      {
        "sensor_id": "soc_0",
        "kind": "soc",
        "source": "sysfs",
        "status": "valid",
        "temperature_milli_celsius": 41250,
        "sampled_at_uptime_ms": 14970
      }
    ],
    "availability": "ready",
    "engine_state": "ready",
    "safety_state": "safe",
    "current_task_id": null,
    "current_attempt_id": null,
    "current_lease_id": null,
    "current_lease_revision": null,
    "accepting_tasks": true,
    "admission_reason": "ready",
    "retry_after_ms": null
  }
}
~~~

### 22.5 task.offer

task.offerにLeaseはなく、このmessageだけではモデルloadまたは推論を開始できない。

~~~json
{
  "protocol_version": "1.0",
  "type": "task.offer",
  "message_id": "b0000005-0000-4000-8000-000000000005",
  "correlation_id": "66666666-6666-4666-8666-666666666666",
  "reply_to_message_id": null,
  "sent_at": "2026-08-18T06:15:10.000Z",
  "session_id": "55555555-5555-4555-8555-555555555555",
  "payload": {
    "task_id": "66666666-6666-4666-8666-666666666666",
    "attempt_id": "77777777-7777-4777-8777-777777777777",
    "offer_valid_for_ms": 10000,
    "task": {
      "task_type": "inference.chat_completion",
      "model_id": "llama_3_2_3b_instruct_q4_k_m",
      "messages": [
        {
          "role": "system",
          "content": "簡潔に回答してください。"
        },
        {
          "role": "user",
          "content": "2と3の和は？"
        }
      ],
      "max_output_tokens": 256,
      "temperature": 0.2,
      "top_p": 0.9,
      "seed": 42,
      "timeout_ms": 120000
    }
  }
}
~~~

### 22.6 task.accept

~~~json
{
  "protocol_version": "1.0",
  "type": "task.accept",
  "message_id": "b0000006-0000-4000-8000-000000000006",
  "correlation_id": "66666666-6666-4666-8666-666666666666",
  "reply_to_message_id": "b0000005-0000-4000-8000-000000000005",
  "sent_at": "2026-08-18T06:15:10.100Z",
  "session_id": "55555555-5555-4555-8555-555555555555",
  "payload": {
    "task_id": "66666666-6666-4666-8666-666666666666",
    "attempt_id": "77777777-7777-4777-8777-777777777777",
    "offer_message_id": "b0000005-0000-4000-8000-000000000005"
  }
}
~~~

### 22.7 attempt.start

このmessageだけが新規実行権限を与える。OrchestratorはLeaseをcommit済みであり、Workerはassignmentを永続化してからEngineを開始する。

~~~json
{
  "protocol_version": "1.0",
  "type": "attempt.start",
  "message_id": "b0000007-0000-4000-8000-000000000007",
  "correlation_id": "66666666-6666-4666-8666-666666666666",
  "reply_to_message_id": "b0000006-0000-4000-8000-000000000006",
  "sent_at": "2026-08-18T06:15:10.200Z",
  "session_id": "55555555-5555-4555-8555-555555555555",
  "payload": {
    "task_id": "66666666-6666-4666-8666-666666666666",
    "attempt_id": "77777777-7777-4777-8777-777777777777",
    "offer_message_id": "b0000005-0000-4000-8000-000000000005",
    "accept_message_id": "b0000006-0000-4000-8000-000000000006",
    "lease_id": "88888888-8888-4888-8888-888888888888",
    "lease_revision": 1,
    "lease_duration_ms": 30000,
    "renew_after_ms": 10000
  }
}
~~~

### 22.8 task.reject

これはprimary Attemptがbusyな間に届いた、別Taskの未掲載task.offer（message ID b0000008-0000-4000-8000-000000000008）への応答である。

~~~json
{
  "protocol_version": "1.0",
  "type": "task.reject",
  "message_id": "b0000009-0000-4000-8000-000000000009",
  "correlation_id": "99999999-9999-4999-8999-999999999999",
  "reply_to_message_id": "b0000008-0000-4000-8000-000000000008",
  "sent_at": "2026-08-18T06:15:12.000Z",
  "session_id": "55555555-5555-4555-8555-555555555555",
  "payload": {
    "task_id": "99999999-9999-4999-8999-999999999999",
    "attempt_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
    "offer_message_id": "b0000008-0000-4000-8000-000000000008",
    "error": {
      "code": "busy",
      "retryable": true,
      "retry_after_ms": 20000
    }
  }
}
~~~

### 22.9 lease.renew

~~~json
{
  "protocol_version": "1.0",
  "type": "lease.renew",
  "message_id": "b000000a-0000-4000-8000-00000000000a",
  "correlation_id": "66666666-6666-4666-8666-666666666666",
  "reply_to_message_id": null,
  "sent_at": "2026-08-18T06:15:20.250Z",
  "session_id": "55555555-5555-4555-8555-555555555555",
  "payload": {
    "task_id": "66666666-6666-4666-8666-666666666666",
    "attempt_id": "77777777-7777-4777-8777-777777777777",
    "lease_id": "88888888-8888-4888-8888-888888888888",
    "lease_revision": 1
  }
}
~~~

### 22.10 lease.renewed

~~~json
{
  "protocol_version": "1.0",
  "type": "lease.renewed",
  "message_id": "b000000b-0000-4000-8000-00000000000b",
  "correlation_id": "66666666-6666-4666-8666-666666666666",
  "reply_to_message_id": "b000000a-0000-4000-8000-00000000000a",
  "sent_at": "2026-08-18T06:15:20.300Z",
  "session_id": "55555555-5555-4555-8555-555555555555",
  "payload": {
    "task_id": "66666666-6666-4666-8666-666666666666",
    "attempt_id": "77777777-7777-4777-8777-777777777777",
    "lease_id": "88888888-8888-4888-8888-888888888888",
    "lease_revision": 2,
    "lease_duration_ms": 30000,
    "renew_after_ms": 10000
  }
}
~~~

### 22.11 attempt.progress

sequenceはattempt.output_chunkと共有する。このeventが欠落してもterminal resultには影響しない。

~~~json
{
  "protocol_version": "1.0",
  "type": "attempt.progress",
  "message_id": "b000000c-0000-4000-8000-00000000000c",
  "correlation_id": "66666666-6666-4666-8666-666666666666",
  "reply_to_message_id": null,
  "sent_at": "2026-08-18T06:15:20.350Z",
  "session_id": "55555555-5555-4555-8555-555555555555",
  "payload": {
    "task_id": "66666666-6666-4666-8666-666666666666",
    "attempt_id": "77777777-7777-4777-8777-777777777777",
    "lease_id": "88888888-8888-4888-8888-888888888888",
    "lease_revision": 2,
    "sequence": 1,
    "stage": "generating",
    "elapsed_ms": 10100,
    "input_tokens": 28,
    "output_tokens": 2,
    "tokens_per_second_milli": 5600
  }
}
~~~

### 22.12 attempt.succeeded

streamに依存しない完全な最終assistant outputを含む。内部思考やchain-of-thoughtは含まない。

~~~json
{
  "protocol_version": "1.0",
  "type": "attempt.succeeded",
  "message_id": "b000000d-0000-4000-8000-00000000000d",
  "correlation_id": "66666666-6666-4666-8666-666666666666",
  "reply_to_message_id": null,
  "sent_at": "2026-08-18T06:15:28.000Z",
  "session_id": "55555555-5555-4555-8555-555555555555",
  "payload": {
    "task_id": "66666666-6666-4666-8666-666666666666",
    "attempt_id": "77777777-7777-4777-8777-777777777777",
    "lease_id": "88888888-8888-4888-8888-888888888888",
    "lease_revision": 2,
    "last_sequence": 1,
    "result": {
      "message": {
        "role": "assistant",
        "content": "5です。"
      },
      "finish_reason": "stop"
    },
    "usage": {
      "input_tokens": 28,
      "output_tokens": 3,
      "total_tokens": 31
    },
    "metrics": {
      "total_duration_ms": 17750,
      "generation_duration_ms": 536,
      "generation_tokens_per_second_milli": 5597
    },
    "completed_at_uptime_ms": 38000
  }
}
~~~

### 22.13 attempt.finalized

OrchestratorはTask、Attempt、Lease、result、dedupe応答を永続化した後にこのACKを送る。上のattempt.succeededが再送されても二度目のcommitを行わず、同じ論理ACKを返す。

~~~json
{
  "protocol_version": "1.0",
  "type": "attempt.finalized",
  "message_id": "b000000e-0000-4000-8000-00000000000e",
  "correlation_id": "66666666-6666-4666-8666-666666666666",
  "reply_to_message_id": "b000000d-0000-4000-8000-00000000000d",
  "sent_at": "2026-08-18T06:15:28.050Z",
  "session_id": "55555555-5555-4555-8555-555555555555",
  "payload": {
    "task_id": "66666666-6666-4666-8666-666666666666",
    "attempt_id": "77777777-7777-4777-8777-777777777777",
    "lease_id": "88888888-8888-4888-8888-888888888888",
    "terminal_message_id": "b000000d-0000-4000-8000-00000000000d",
    "disposition": "committed",
    "canonical_terminal_type": "attempt.succeeded",
    "attempt_state": "succeeded",
    "task_state": "succeeded"
  }
}
~~~

## 23. Conformance scenarios

実装は少なくとも次のscenarioを、正本状態、Engine起動回数、永続化record、送受信message、UI表示まで含めて検証しなければならない（MUST）。

| Scenario | 必須確認事項 |
|---|---|
| 正常な接続・登録・Heartbeat | WSS認証、hello/welcome、describe/ack、initial Heartbeat/ack、readyの順序を守り、ready前にTaskを割り当てない。Orchestratorが受信monotonic時刻からonlineを導出する |
| Taskの正常完了 | offer、accept、Lease永続化、start、assignment永続化、Engine開始、started、succeeded、terminal永続化、finalizedとなり、Task/Attemptを一度だけsucceededにする |
| busyによる拒否 | task.rejectのbusyを返し、slot、Engine、Attempt executionを増やさない |
| coolingによる拒否 | cooling_downとretryable、見積可能ならretry_after_msを返し、Engineを開始しない |
| duplicate offer | 同じ論理応答を返し、予約slot、busy count、Attempt数を増やさない |
| duplicate start | active assignmentとEngine processが一つだけで、二重推論を開始しない |
| message ID競合 | 同じmessage IDと異なるsemantic payloadをmessage_id_conflictとし、状態を変更しない |
| terminal ACK消失後の再送 | Workerが同じterminal message IDとpayloadを再送し、Orchestratorが同じfinalizedを返し、terminal commitが一つだけである。再接続を挟み未知eventだった場合、完全一致するterminal-only rebindより前にLeaseをrevokeしない |
| 推論中切断とLease失効 | Heartbeatやsocket activityでLeaseを更新せず、renewedを期限内に得られないWorkerがmonotonic deadlineまでにEngineを停止する |
| reconnectのidle | active assignmentがなく、describe/heartbeat後にreadyへ復帰する |
| reconnectのcontinue | 同一boot、Task、Attempt、Lease、revisionが一致し、両方のdeadlineが未失効の場合だけ既存Engineを継続する。welcome自体ではLeaseを延長せず、専用renewalを行う |
| reconnectのstop | stale/unknown assignmentを再開せず、cooperative stopと必要なforce killを行い、旧Leaseを復活させない |
| reconnectのquarantine | ID衝突または永続化矛盾時にEngineとTask受付を停止し、自動的にidleへfallbackしない |
| Attempt認識不一致 | Workerだけactiveならstop、両者が異なるactive Attemptならquarantine、Orchestrator正本を自己申告で上書きしない |
| stale Sessionのfencing | 新Session commit後、旧SessionのHeartbeat、renew、terminal eventをstale_sessionとし、liveness、Lease、Taskを変更しない |
| accept後・start前の切断 | 旧Session-bound reservationを解放してAttemptをrevokedとし、旧acceptを新Sessionのstart根拠にせず、新attempt_idでretryする |
| Worker再起動 | 新boot_idで旧monotonic deadlineを復元せず、永続assignmentを自動実行せず、stop reconciliationを待つ |
| Orchestrator再起動後の復元 | Task、Attempt、Lease、Cancellation、terminal、dedupe responseを復元し、pre-restart Leaseをfenceしてからdispatchを再開する |
| CompletionとCancellationの競合 | Orchestratorで先にtransaction commitされた側だけをTask正本にし、sent_atやcancel_ack順で決めない |
| offer期限後のaccept | stale_attemptとしてLeaseを発行せず、Engineを開始しない |
| Lease更新ACK消失 | 同じlease.renewを再送するとrevisionを再増加せず同じlease.renewedを返す |
| Lease revision不一致 | renewal requestの逆行、未発行、別Leaseのrevisionをinvalid_lease_revisionとし、expired/revoked Leaseを復活させない |
| suspend/resume | sleep時間をLeaseとsensor freshnessのelapsed timeへ含め、clock不連続時はLease expired・required sensor staleとしてfail-closedにする |
| 未知optional field | 同一MAJORでは無視し、既知fieldの処理結果を変えない |
| 未知required enum | schema_violationを返し、推測またはfallback実行をしない |
| malformed JSON | syntax error、invalid UTF-8、duplicate key、NaN/Infinity相当、過剰nestingを拒否し、状態を変更しない |
| malformed UDP discovery | 応答を返さずdropし、WSS認証済み状態へ影響させない |
| oversized JSON | type別上限の境界を検証し、超過をbounded memoryで拒否する |
| binary WebSocket message | binary_frameまたはcloseとして拒否し、Taskとして処理しない |
| credential不一致 | WSS upgrade前に拒否し、SessionまたはLeaseを作らない |
| Node ID不一致 | credentialへbindされたnode_idとhelloのnode_idが違えばsession.rejectし、旧Sessionをfenceしない |
| 公開鍵pin不一致 | WorkerがTLS接続を中止し、Authorization credentialを送信しない |
| Orchestratorなしでのthermal abort | 切断中もpause、critical、sensor staleness、cooldown、Engine force killが動作する |
| stale required sensor | fail-closedとなり、新Taskを拒否し、実行中Attemptをローカル中断する |
| stream chunk欠落 | sequence gapを許容し、chunkからfinalを再構築せず、attempt.succeededの完全resultだけを採用する |
| stale Attemptのstream | chunkをDashboardへ表示せず、Task正本やfinal resultへ混入させない |
| backpressureとrate limit | progress/chunkだけを集約・破棄でき、Lease、Cancel、control、terminal resultを黙って捨てない |
| 禁止Task面 | shell等のtask_typeはunsupported_taskで拒否する。inference.chat_completion内の未知fieldは共通規約どおり無視し、shell、file、URL、tool callとして一切作用させず、prompt内文字列も実行・fetchしない |
| hidden reasoning除外 | Engineがreasoning channelを返しても通信、永続化、CUI、Dashboardに現れない |

## 自己レビュー

- offlineはWorkerの自己申告enumまたはHeartbeatに含めず、Orchestratorのmonotonic受信時刻からだけ導出している。
- Heartbeat、ping/pong、progress、session.welcomeはLeaseを暗黙更新せず、lease.renew / lease.renewedだけを更新経路としている。
- task.offerとtask.acceptだけでは推論を開始せず、永続化済みattempt.startだけを新規実行権限としている。
- Lease更新失敗、失効、revoke、Worker再起動後にWorkerが実行を継続または自動再開しない。
- 新Sessionのwelcome commitが旧Sessionをfenceし、旧Sessionのmessageが正本を変更しない。
- 未ACK terminal resultは同じmessage IDとpayloadで再送され、永続dedupeと同じattempt.finalizedにより二重commitを起こさない。
- Workerのrequired sensor、安全閾値、cooldown、強制終了hard limitをOrchestratorから緩和するmessageを定義していない。
- 任意コード実行、任意shell、任意URL取得、任意ファイルアクセス、model download/transferの経路を定義していない。
- hidden chain-of-thoughtを要求、通信、保存、表示するfieldを定義していない。
