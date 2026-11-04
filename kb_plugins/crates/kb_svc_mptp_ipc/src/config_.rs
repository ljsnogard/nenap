//! 连接类型与常量。

use buffex_compio_adapt::{BuffRead, BuffWrite, DefaultAllocConfig};
use compio::net::UnixStream;
use rpc_core_v1::{ClientConfig, ServingConfig};
use smux_v1::connection::{DefaultConnCfg, MuxConnection};

/// socket 写入半边（环 → socket）。
pub type Tx = BuffWrite<UnixStream>;

/// socket 读取半边（socket → 环）。
pub type Rx = BuffRead<UnixStream>;

/// 本机复用连接。
pub type Conn = MuxConnection<DefaultConnCfg<Tx, Rx>>;

/// 客户端配置。
pub type CliCfg = ClientConfig<Conn>;

/// 服务端配置。
pub type SrvCfg = ServingConfig<Conn>;

/// 协议约定的默认服务垛口。
pub const K_SERVING_DOCK: u32 = 0x80;

/// 监听循环的入向邀请上限。
pub const K_LISTEN_RESERVE: usize = 8;

/// socket 适配器两侧环的容量。
pub const K_RING_CAP: usize = 64 * 1024;

/// 把一条 UDS 流拆成两个适配器半边。
pub(crate) fn adapt_(stream: UnixStream) -> std::io::Result<(Tx, Rx)> {
    let (writer, reader) = stream.into_split();
    let rx = Rx::try_new(reader, K_RING_CAP, DefaultAllocConfig)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let tx = Tx::try_new(writer, K_RING_CAP, DefaultAllocConfig)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    Ok((tx, rx))
}
