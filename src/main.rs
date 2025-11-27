// Tratamento de erros ergonômico
use anyhow::{Context, Result};

// Criação e configuração de sockets de baixo nível
use socket2::{Domain, Protocol, Socket, Type};

// Erros de I/O (timeout, would-block, etc.)
use std::io::{self, Read};

// Endereços (IPv4, socket address)
use std::net::{IpAddr, SocketAddr};

// Medição de tempo (RTT)
use std::time::{Duration, Instant};

// Módulos locais
mod args;
mod icmp;

/// Programa principal: envia Echo Request e aguarda Echo Reply.
/// Requer privilégios de Administrador no Windows (Raw Sockets).
fn main() -> Result<()> {
    // Parsing de argumentos via módulo args
    let args = args::parse()?;
    let dst = args.dst;
    let count = args.count;

    // Configura handler para Ctrl+C
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, std::sync::atomic::Ordering::SeqCst);
    })
    .context("Erro ao configurar handler de Ctrl+C")?;

    // Cria um socket ICMP RAW
    // Domain::IPV4 -> AF_INET
    // Type::RAW -> SOCK_RAW (Necessário no Windows para ICMP)
    // Protocol::ICMPV4 -> IPPROTO_ICMP
    // Nota: SOCK_RAW é 3. Usamos o valor direto pois libc::SOCK_RAW pode não estar disponível no Windows.
    let mut sock = Socket::new(
        Domain::IPV4,
        Type::from(3),
        Some(Protocol::ICMPV4),
    )
    .context("Falha ao criar socket RAW. Verifique se está rodando como Administrador.")?;

    // Timeout de leitura de 2s
    sock.set_read_timeout(Some(Duration::from_secs(2)))?;

    // Endereço de destino (porta 0 é ignorada para ICMP)
    let dst_sa = SocketAddr::new(IpAddr::V4(dst), 0);

    // Identificador: usa o PID do processo (comum em pings)
    let ident: u16 = std::process::id() as u16;

    // Payload enviado dentro do pacote ICMP
    let payload = b"pingrs-windows";

    println!("Disparando {} com {} bytes de dados:", dst, payload.len());

    let mut seq = 1u16;
    let mut sent_count = 0u64;

    // Estatísticas
    let mut transmitted = 0u64;
    let mut received_count = 0u64;
    let mut rtts = Vec::new();

    loop {
        // Verifica se foi interrompido
        if !running.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }

        // Verifica limite de contagem se houver
        if let Some(limit) = count {
            if sent_count >= limit {
                break;
            }
        }

        // Constrói o Echo Request via módulo icmp
        let pkt = icmp::build_echo_request(ident, seq, payload);

        // Marca o instante do envio para calcular o RTT depois
        let t0 = Instant::now();

        // Incrementa transmitidos (tentativa)
        transmitted += 1;

        // Envia o Echo Request
        let send_result = sock.send_to(&pkt, &dst_sa.into());
        if let Err(e) = send_result {
            println!("Falha ao enviar: {}", e);
            // Se falhou ao enviar, não aguardamos resposta, mas conta como perda.
            // Apenas dormimos e vamos para o próximo.
        } else {
            // Buffer de recepção (MTU típica)
            let mut buf = [0u8; 1500];

            // Loop de recepção para este pacote específico (com timeout de 2s)
            let deadline = t0 + Duration::from_secs(2);

            loop {
                if Instant::now() >= deadline {
                    println!("Esgotado o tempo limite do pedido.");
                    break;
                }

                // Verifica interrupção também no loop de espera
                if !running.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }

                // Usando `read` do std::io::Read
                let n = match sock.read(&mut buf) {
                    Ok(n) => n,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => {
                        continue;
                    }
                    Err(e) => {
                        println!("Erro na leitura: {}", e);
                        break;
                    }
                };

                // Alguns SOs podem incluir o cabeçalho IP no início; detecta IPv4 e pula IHL se for o caso
                let start = if n >= 20 && (buf[0] >> 4) == 4 {
                    let ihl = (buf[0] & 0x0F) as usize * 4;
                    ihl
                } else {
                    0
                };

                if n < start + 8 { continue; }

                let icmp = &buf[start..n];
                let icmp_type = icmp[0]; // 0 = Echo Reply
                let icmp_code = icmp[1]; // 0
                let r_id = u16::from_be_bytes([icmp[4], icmp[5]]);
                let r_seq = u16::from_be_bytes([icmp[6], icmp[7]]);

                if icmp_type == 0 && icmp_code == 0 && r_id == ident && r_seq == seq {
                    let rtt_ms = t0.elapsed().as_secs_f64() * 1000.0;
                    println!(
                        "Resposta de {}: bytes={} icmp_seq={} tempo={:.2}ms",
                        dst, n - start, seq, rtt_ms
                    );
                    received_count += 1;
                    rtts.push(rtt_ms);
                    break;
                }
            }
        }
        sent_count += 1;

        // Verifica interrupção antes do sleep
        if !running.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }

        // Prepara próxima sequência (wrap around)
        seq = seq.wrapping_add(1);
        if seq == 0 { seq = 1; }

        // Sleep de 1s entre pings
        // Se tiver limite, não dorme depois do último
        if let Some(limit) = count {
            if sent_count < limit {
                std::thread::sleep(Duration::from_secs(1));
            }
        } else {
            // Infinito: sempre dorme
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    // Exibe estatísticas ao sair
    println!("\n--- estatísticas de ping para {} ---", dst);
    let loss = if transmitted > 0 {
        (transmitted - received_count) as f64 / transmitted as f64 * 100.0
    } else {
        0.0
    };
    println!(
        "{} pacotes transmitidos, {} recebidos, {:.0}% de perda de pacotes",
        transmitted, received_count, loss
    );

    if !rtts.is_empty() {
        let min = rtts.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = rtts.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let sum: f64 = rtts.iter().sum();
        let avg = sum / rtts.len() as f64;
        println!(
            "rtt min/avg/max = {:.3}/{:.3}/{:.3} ms",
            min, avg, max
        );
    }

    Ok(())
}
