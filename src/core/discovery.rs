use os_release::OsRelease;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OsType {
    Arch,
    Ubuntu,
    Debian,
    Fedora,
    OpenSuse,
    Unknown(String),
}

pub fn detect_os() -> OsType {
    match OsRelease::new() {
        Ok(release) => match release.id.as_str() {
            "arch" | "manjaro" | "endeavouros" => OsType::Arch,
            "ubuntu" | "pop" | "mint" => OsType::Ubuntu,
            "debian" => OsType::Debian,
            "fedora" | "centos" | "rhel" => OsType::Fedora,
            "opensuse" | "opensuse-tumbleweed" | "opensuse-leap" | "suse" => OsType::OpenSuse,
            other => OsType::Unknown(other.to_string()),
        },
        Err(_) => OsType::Unknown("unknown".to_string()),
    }
}
