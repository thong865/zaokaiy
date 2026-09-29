use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    pub jwt_secret: String,
    pub jwt_ttl_hours: i64,
    pub cors_origins: Vec<String>,
    /// Anthropic API key. When empty, AI endpoints fall back to built-in templates.
    pub anthropic_api_key: Option<String>,
    pub ai_model: String,
    pub anthropic_base_url: String,
    // Media storage
    pub media_driver: String,
    pub media_dir: String,
    pub media_public_url: String,
    pub media_max_image_mb: usize,
    pub media_max_video_mb: usize,
    /// Minimum px on the shortest side of an uploaded image (0 = no check).
    pub media_min_image_edge: u32,
    pub s3_endpoint: String,
    pub s3_bucket: String,
    pub s3_region: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    // Moderation
    /// Products need admin approval before going live (PRODUCT_REVIEW=required|off).
    pub review_required: bool,
    /// Editing an approved product's content (name, description, category, media) re-queues it.
    pub review_on_edit: bool,
    /// Users with these emails are promoted to admin at startup/registration.
    pub admin_emails: Vec<String>,
    // Social commerce
    /// Public URL of the Nuxt site (used in checkout links sent to customers).
    pub public_web_url: String,
    pub meta_verify_token: String,
    pub meta_app_secret: String,
    pub meta_graph_url: String,
    pub tiktok_client_secret: String,
    // Social login
    /// Public URL of this API as browsers/providers reach it (OAuth callback base), e.g. https://shop.example.com/api
    pub public_api_url: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_auth_url: String,
    pub google_token_url: String,
    pub google_userinfo_url: String,
    pub facebook_app_id: String,
    pub facebook_app_secret: String,
    pub facebook_dialog_url: String,
    /// WhatsApp Cloud API credentials for login codes (platform number, not a shop's).
    pub whatsapp_token: String,
    pub whatsapp_phone_number_id: String,
    pub whatsapp_otp_template: String,
    pub whatsapp_otp_lang: String,
    pub whatsapp_otp_lang_lo: String,
    /// Return the WhatsApp code in the API response (local testing only — never in production).
    pub otp_dev_echo: bool,
    /// Seconds before another WhatsApp code can be requested for the same number.
    pub otp_resend_secs: f64,
    // Corporate KYC
    /// Business shops must be verified before publishing products (KYB_REQUIRED_TO_PUBLISH=true|false).
    pub kyb_required_to_publish: bool,
    /// Months until an approved verification is due for review again.
    pub kyb_review_months: u32,
    /// 32-byte key (base64 or hex) encrypting KYC documents and ID / bank numbers.
    pub kyc_encryption_key: String,
    /// Local directory for private files (KYC documents). Must NOT be publicly served.
    pub private_dir: String,
    /// S3 bucket for private files (defaults to S3_BUCKET with the `private/` prefix). Keep it non-public.
    pub kyc_s3_bucket: String,
    // Cloudflare Turnstile (human check on sign-in / sign-up). Off when either key is empty.
    pub turnstile_site_key: String,
    pub turnstile_secret_key: String,
}

impl Config {
    pub fn from_env() -> Self {
        let get = |k: &str, d: &str| env::var(k).unwrap_or_else(|_| d.to_string());
        Self {
            database_url: get(
                "DATABASE_URL",
                "postgres://zaokaiy:zaokaiy@localhost:5432/zaokaiy",
            ),
            bind_addr: get("BIND_ADDR", "0.0.0.0:8080"),
            jwt_secret: get("JWT_SECRET", "dev-secret-change-me"),
            jwt_ttl_hours: get("JWT_TTL_HOURS", "168").parse().unwrap_or(168),
            cors_origins: get("CORS_ORIGINS", "http://localhost:3000")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            anthropic_api_key: env::var("ANTHROPIC_API_KEY")
                .ok()
                .filter(|k| !k.trim().is_empty()),
            ai_model: get("AI_MODEL", "claude-sonnet-5"),
            anthropic_base_url: get("ANTHROPIC_BASE_URL", "https://api.anthropic.com"),
            media_driver: get("MEDIA_DRIVER", "local").to_lowercase(),
            media_dir: get("MEDIA_DIR", "./media"),
            media_public_url: get("MEDIA_PUBLIC_URL", "http://localhost:8080/media"),
            media_max_image_mb: get("MEDIA_MAX_IMAGE_MB", "15").parse().unwrap_or(15),
            media_max_video_mb: get("MEDIA_MAX_VIDEO_MB", "200").parse().unwrap_or(200),
            media_min_image_edge: get("MEDIA_MIN_IMAGE_EDGE", "500").parse().unwrap_or(500),
            s3_endpoint: get("S3_ENDPOINT", ""),
            s3_bucket: get("S3_BUCKET", "zaokaiy-media"),
            s3_region: get("S3_REGION", "us-east-1"),
            s3_access_key: get("S3_ACCESS_KEY", ""),
            s3_secret_key: get("S3_SECRET_KEY", ""),
            review_required: get("PRODUCT_REVIEW", "required").to_lowercase() != "off",
            review_on_edit: get("REVIEW_ON_EDIT", "true").to_lowercase() != "false",
            admin_emails: get("ADMIN_EMAILS", "")
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect(),
            public_web_url: get("PUBLIC_WEB_URL", "http://localhost:3000"),
            meta_verify_token: get("META_VERIFY_TOKEN", ""),
            meta_app_secret: get("META_APP_SECRET", ""),
            meta_graph_url: get("META_GRAPH_URL", "https://graph.facebook.com/v21.0"),
            tiktok_client_secret: get("TIKTOK_CLIENT_SECRET", ""),
            public_api_url: get("PUBLIC_API_URL", "http://localhost:8080/api"),
            google_client_id: get("GOOGLE_CLIENT_ID", ""),
            google_client_secret: get("GOOGLE_CLIENT_SECRET", ""),
            google_auth_url: get("GOOGLE_AUTH_URL", "https://accounts.google.com/o/oauth2/v2/auth"),
            google_token_url: get("GOOGLE_TOKEN_URL", "https://oauth2.googleapis.com/token"),
            google_userinfo_url: get("GOOGLE_USERINFO_URL", "https://openidconnect.googleapis.com/v1/userinfo"),
            facebook_app_id: get("FACEBOOK_APP_ID", ""),
            facebook_app_secret: get("FACEBOOK_APP_SECRET", ""),
            facebook_dialog_url: get("FACEBOOK_DIALOG_URL", "https://www.facebook.com/v21.0/dialog/oauth"),
            whatsapp_token: get("WHATSAPP_TOKEN", ""),
            whatsapp_phone_number_id: get("WHATSAPP_PHONE_NUMBER_ID", ""),
            whatsapp_otp_template: get("WHATSAPP_OTP_TEMPLATE", "zaokaiy_login"),
            whatsapp_otp_lang: get("WHATSAPP_OTP_LANG", "en_US"),
            whatsapp_otp_lang_lo: get("WHATSAPP_OTP_LANG_LO", ""),
            otp_dev_echo: get("OTP_DEV_ECHO", "false").to_lowercase() == "true",
            otp_resend_secs: get("OTP_RESEND_SECONDS", "60").parse().unwrap_or(60.0_f64).max(1.0),
            kyb_required_to_publish: get("KYB_REQUIRED_TO_PUBLISH", "true").to_lowercase() != "false",
            kyb_review_months: get("KYB_REVIEW_MONTHS", "12").parse().unwrap_or(12).clamp(1, 120),
            kyc_encryption_key: get("KYC_ENCRYPTION_KEY", ""),
            private_dir: get("PRIVATE_DIR", "./private"),
            kyc_s3_bucket: get("KYC_S3_BUCKET", ""),
            turnstile_site_key: get("TURNSTILE_SITE_KEY", "").trim().to_string(),
            turnstile_secret_key: get("TURNSTILE_SECRET_KEY", "").trim().to_string(),
        }
    }
}
