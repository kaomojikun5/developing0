use pocketswarm_protocol::ProtocolVersion;

#[test]
fn constructs_and_reads_components() {
    let version = ProtocolVersion::new(1, 42);

    assert_eq!(version.major(), 1);
    assert_eq!(version.minor(), 42);
}

#[test]
fn mvp_version_is_1_0() {
    let version = ProtocolVersion::V1_0;

    assert_eq!(version.major(), 1);
    assert_eq!(version.minor(), 0);
    assert_eq!(version, ProtocolVersion::new(1, 0));
}

#[test]
fn displays_as_major_dot_minor() {
    assert_eq!(ProtocolVersion::V1_0.to_string(), "1.0");
    assert_eq!(ProtocolVersion::new(9, 42).to_string(), "9.42");
    assert_eq!(ProtocolVersion::new(0, 0).to_string(), "0.0");
}
