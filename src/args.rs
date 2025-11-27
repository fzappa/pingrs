use anyhow::{Context, Result};
use std::net::Ipv4Addr;

pub struct PingArgs {
    pub dst: Ipv4Addr,
    pub count: Option<u64>,
}

pub fn parse() -> Result<PingArgs> {
    let args: Vec<String> = std::env::args().collect();
    let mut dst_str = None;
    let mut count = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-c" => {
                if i + 1 < args.len() {
                    let c: u64 = args[i + 1].parse().context("Valor inválido para -c")?;
                    count = Some(c);
                    i += 1;
                } else {
                    anyhow::bail!("Faltou o valor para -c");
                }
            }
            val => {
                if dst_str.is_none() {
                    dst_str = Some(val);
                }
            }
        }
        i += 1;
    }

    let dst_str = dst_str.context("Uso: pingrs <ipv4> [-c <count>]")?;
    let dst: Ipv4Addr = dst_str.parse().context("Endereço IP inválido")?;

    Ok(PingArgs { dst, count })
}
