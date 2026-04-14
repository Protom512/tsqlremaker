//! サーバー設定

/// Language Server の設定
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// ログレベル
    pub log_level: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
        }
    }
}

impl ServerConfig {
    /// 設定を適用してロガーを初期化する
    pub fn init_logging(&self) {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&self.log_level)),
            )
            .with_writer(std::io::stderr)
            .init();
    }
}
