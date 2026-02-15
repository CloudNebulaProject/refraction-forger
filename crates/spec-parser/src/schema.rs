use knuffel::Decode;

/// Distro family derived from the `distro` string in a spec.
/// Not KDL-decoded directly — computed via `from_distro_str`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DistroFamily {
    #[default]
    OmniOS,
    Ubuntu,
}

impl DistroFamily {
    pub fn from_distro_str(s: Option<&str>) -> Self {
        match s {
            Some(d) if d.starts_with("ubuntu") => DistroFamily::Ubuntu,
            _ => DistroFamily::OmniOS,
        }
    }
}

#[derive(Debug, Decode)]
pub struct ImageSpec {
    #[knuffel(child)]
    pub metadata: Metadata,

    #[knuffel(child, unwrap(argument))]
    pub distro: Option<String>,

    #[knuffel(child, unwrap(argument))]
    pub base: Option<String>,

    #[knuffel(child, unwrap(argument))]
    pub build_host: Option<String>,

    #[knuffel(child)]
    pub repositories: Repositories,

    #[knuffel(child, unwrap(argument))]
    pub incorporation: Option<String>,

    #[knuffel(child)]
    pub variants: Option<Variants>,

    #[knuffel(child)]
    pub certificates: Option<Certificates>,

    #[knuffel(children(name = "packages"))]
    pub packages: Vec<PackageList>,

    #[knuffel(children(name = "customization"))]
    pub customizations: Vec<Customization>,

    #[knuffel(children(name = "overlays"))]
    pub overlays: Vec<Overlays>,

    #[knuffel(children(name = "include"))]
    pub includes: Vec<Include>,

    #[knuffel(children(name = "target"))]
    pub targets: Vec<Target>,

    #[knuffel(child)]
    pub builder: Option<BuilderNode>,
}

/// Configuration for a builder VM used when the host can't build locally.
#[derive(Debug, Decode)]
pub struct BuilderNode {
    #[knuffel(child, unwrap(argument))]
    pub image: Option<String>,

    #[knuffel(child, unwrap(argument))]
    pub vcpus: Option<u16>,

    #[knuffel(child, unwrap(argument))]
    pub memory: Option<u64>,

    #[knuffel(child, unwrap(argument))]
    pub disk: Option<u32>,
}

#[derive(Debug, Decode)]
pub struct Metadata {
    #[knuffel(property)]
    pub name: String,
    #[knuffel(property)]
    pub version: String,
    #[knuffel(property)]
    pub description: Option<String>,
}

#[derive(Debug, Decode)]
pub struct Repositories {
    #[knuffel(children(name = "publisher"))]
    pub publishers: Vec<Publisher>,

    #[knuffel(children(name = "apt-mirror"))]
    pub apt_mirrors: Vec<AptMirror>,
}

#[derive(Debug, Decode)]
pub struct AptMirror {
    #[knuffel(argument)]
    pub url: String,
    #[knuffel(property)]
    pub suite: String,
    #[knuffel(property)]
    pub components: Option<String>,
}

#[derive(Debug, Decode)]
pub struct Publisher {
    #[knuffel(property)]
    pub name: String,
    #[knuffel(property)]
    pub origin: String,
}

#[derive(Debug, Decode)]
pub struct PackageList {
    #[knuffel(property)]
    pub r#if: Option<String>,

    #[knuffel(children(name = "package"))]
    pub packages: Vec<Package>,
}

#[derive(Debug, Decode)]
pub struct Package {
    #[knuffel(argument)]
    pub name: String,
}

#[derive(Debug, Decode)]
pub struct Customization {
    #[knuffel(property)]
    pub r#if: Option<String>,

    #[knuffel(children(name = "user"))]
    pub users: Vec<User>,
}

#[derive(Debug, Decode)]
pub struct User {
    #[knuffel(argument)]
    pub name: String,
}

#[derive(Debug, Decode)]
pub struct Overlays {
    #[knuffel(property)]
    pub r#if: Option<String>,

    #[knuffel(children)]
    pub actions: Vec<OverlayAction>,
}

#[derive(Debug, Decode)]
pub enum OverlayAction {
    File(FileOverlay),
    Devfsadm(Devfsadm),
    EnsureDir(EnsureDir),
    RemoveFiles(RemoveFiles),
    EnsureSymlink(EnsureSymlink),
    Shadow(ShadowOverlay),
}

#[derive(Debug, Decode)]
pub struct FileOverlay {
    #[knuffel(property)]
    pub destination: String,

    #[knuffel(property)]
    pub source: Option<String>,

    #[knuffel(property)]
    pub owner: Option<String>,
    #[knuffel(property)]
    pub group: Option<String>,
    #[knuffel(property)]
    pub mode: Option<String>,
}

#[derive(Debug, Decode)]
pub struct Devfsadm {}

#[derive(Debug, Decode)]
pub struct EnsureDir {
    #[knuffel(argument)]
    pub path: String,
    #[knuffel(property)]
    pub owner: Option<String>,
    #[knuffel(property)]
    pub group: Option<String>,
    #[knuffel(property)]
    pub mode: Option<String>,
}

#[derive(Debug, Decode)]
pub struct RemoveFiles {
    #[knuffel(property)]
    pub file: Option<String>,
    #[knuffel(property)]
    pub dir: Option<String>,
    #[knuffel(property)]
    pub pattern: Option<String>,
}

#[derive(Debug, Decode)]
pub struct EnsureSymlink {
    #[knuffel(argument)]
    pub path: String,
    #[knuffel(property)]
    pub target: String,
    #[knuffel(property)]
    pub owner: Option<String>,
    #[knuffel(property)]
    pub group: Option<String>,
}

#[derive(Debug, Decode)]
pub struct ShadowOverlay {
    #[knuffel(property)]
    pub username: String,
    #[knuffel(property)]
    pub password: String,
}

#[derive(Debug, Decode)]
pub struct Target {
    #[knuffel(argument)]
    pub name: String,

    #[knuffel(property, str)]
    pub kind: TargetKind,

    #[knuffel(child, unwrap(argument))]
    pub disk_size: Option<String>,

    #[knuffel(child, unwrap(argument))]
    pub bootloader: Option<String>,

    #[knuffel(child, unwrap(argument))]
    pub filesystem: Option<String>,

    #[knuffel(child, unwrap(argument))]
    pub push_to: Option<String>,

    #[knuffel(child)]
    pub entrypoint: Option<Entrypoint>,

    #[knuffel(child)]
    pub environment: Option<Environment>,

    #[knuffel(child)]
    pub pool: Option<Pool>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub enum TargetKind {
    Qcow2,
    Oci,
    #[default]
    Artifact,
}

impl std::str::FromStr for TargetKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "qcow2" | "qcow" => Ok(TargetKind::Qcow2),
            "oci" => Ok(TargetKind::Oci),
            "artifact" | "tar" => Ok(TargetKind::Artifact),
            other => Err(format!("invalid target kind: {other}")),
        }
    }
}

impl std::fmt::Display for TargetKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TargetKind::Qcow2 => write!(f, "qcow2"),
            TargetKind::Oci => write!(f, "oci"),
            TargetKind::Artifact => write!(f, "artifact"),
        }
    }
}

#[derive(Debug, Decode)]
pub struct Entrypoint {
    #[knuffel(property)]
    pub command: String,
}

#[derive(Debug, Decode)]
pub struct Environment {
    #[knuffel(children(name = "set"))]
    pub vars: Vec<EnvVar>,
}

#[derive(Debug, Decode)]
pub struct EnvVar {
    #[knuffel(argument)]
    pub key: String,
    #[knuffel(argument)]
    pub value: String,
}

#[derive(Debug, Decode)]
pub struct Pool {
    #[knuffel(children(name = "property"))]
    pub properties: Vec<PoolProperty>,
}

#[derive(Debug, Decode)]
pub struct PoolProperty {
    #[knuffel(property)]
    pub name: String,
    #[knuffel(property)]
    pub value: String,
}

#[derive(Debug, Decode)]
pub struct Variants {
    #[knuffel(children(name = "set"))]
    pub vars: Vec<VariantPair>,
}

#[derive(Debug, Decode)]
pub struct VariantPair {
    #[knuffel(property)]
    pub name: String,
    #[knuffel(property)]
    pub value: String,
}

#[derive(Debug, Decode)]
pub struct Certificates {
    #[knuffel(children(name = "ca"))]
    pub ca: Vec<CaCertificate>,
}

#[derive(Debug, Decode)]
pub struct CaCertificate {
    #[knuffel(property)]
    pub publisher: String,
    #[knuffel(property)]
    pub certfile: String,
}

#[derive(Debug, Decode)]
pub struct Include {
    #[knuffel(argument)]
    pub path: String,
}
