//! 工作区目录浏览。
//!
//! 对应桌面客户端右侧的「工作区文件」面板。界面骨架已经在客户端里立好了，
//! 只等这条通道接上。

use super::ids_::WorkspaceId;

/// 目录条目的类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
#[denco(rename_all = "snake_case")]
pub enum DirEntryKind {
    /// 普通文件。
    File,

    /// 目录。
    Directory,

    /// 其它（符号链接、设备等）。
    Other,
}

/// 目录里的一个条目。
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct DirEntry {
    /// 条目名（不含父路径）。
    pub name: String,

    /// 条目类型。
    pub kind: DirEntryKind,

    /// 文件大小时为 `Some`；目录通常为 `None`。
    pub size_bytes: Option<u64>,
}

/// 一次目录列举的结果。
#[derive(Debug, Clone, PartialEq, Eq, denco_macro::TrEncodable, denco_macro::TrDecodable)]
pub struct DirectoryListing {
    /// 所属工作区。
    pub workspace_id: WorkspaceId,

    /// 相对于工作区根目录的路径；根目录用空串表示。
    pub relative_path: String,

    /// 该目录下的条目。
    pub entries: Vec<DirEntry>,
}
