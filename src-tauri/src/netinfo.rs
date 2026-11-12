//! 本机网络信息探测（供设置页展示「可连接」的 API / WebSocket 终结点）。
//!
//! 局域网模式（`0.0.0.0`）下，绑定的地址不是可连接地址：用户需要知道
//! 本机在局域网中的真实 IP，才能把终结点填到别的设备上。这里用零依赖的方式探测。

use std::net::{IpAddr, Ipv4Addr, UdpSocket};

/// 探测默认出口网卡的局域网 IPv4 地址。
///
/// 做法：对一个外部地址做 UDP `connect`（UDP 无握手，不会真的发包），
/// 再由 `local_addr()` 读出内核按路由表选中的本机地址。
/// 因此**不需要外网可达**，只要存在一条默认路由即可；完全没有网络时返回 `None`。
///
/// 局限：返回的是「默认路由出口网卡」的地址。装了 Hyper-V / VMware / Docker 等
/// 虚拟网卡时，出口可能是虚拟网卡（如 `172.20.x.1`），未必是其他设备能访问的那张网卡。
/// 用户仍可在设置页手填具体 IP 覆盖；此处只负责给出最常见的正确答案。
pub fn local_ipv4() -> Option<Ipv4Addr> {
    // 两个目标互为备份：一个不可达时内核仍会按默认路由给出出口地址
    const PROBES: [&str; 2] = ["8.8.8.8:80", "223.5.5.5:53"];
    for probe in PROBES {
        let sock = match UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(_) => continue,
        };
        if sock.connect(probe).is_err() {
            continue;
        }
        if let Ok(addr) = sock.local_addr() {
            if let IpAddr::V4(ip) = addr.ip() {
                if is_lan_ipv4(ip) {
                    return Some(ip);
                }
            }
        }
    }
    None
}

/// 是否为适合对外提供服务的局域网地址。
///
/// 排除回环、未指定、广播、链路本地（169.254.x.x，通常是 DHCP 失败）。
pub fn is_lan_ipv4(ip: Ipv4Addr) -> bool {
    !(ip.is_loopback() || ip.is_unspecified() || ip.is_broadcast() || ip.is_link_local())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_routable_addresses() {
        for raw in ["127.0.0.1", "0.0.0.0", "169.254.10.20"] {
            let ip: Ipv4Addr = raw.parse().unwrap();
            assert!(!is_lan_ipv4(ip), "{} 不应被当作可服务地址", raw);
        }
    }

    #[test]
    fn accepts_common_lan_addresses() {
        for raw in ["192.168.1.5", "10.0.0.12", "172.16.31.4", "100.64.0.7", "8.8.8.8"] {
            let ip: Ipv4Addr = raw.parse().unwrap();
            assert!(is_lan_ipv4(ip), "{} 应被接受", raw);
        }
    }

    #[test]
    fn probe_never_returns_non_routable_address() {
        // 结果取决于运行环境：可能探测不到（CI 无默认路由），
        // 但一旦探测到，绝不能是回环/链路本地这类不可用地址。
        if let Some(ip) = local_ipv4() {
            assert!(is_lan_ipv4(ip), "探测到不可用地址: {ip}");
        }
    }
}
