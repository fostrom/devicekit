mod cli;
mod http_server;
mod moonlight_codec;
mod moonlight_socket;
mod notifycast;
mod telemetry;

fn main() {
    cli::exec();
}
