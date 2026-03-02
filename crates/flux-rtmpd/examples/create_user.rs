/// 示例：创建 RTMP 用户
/// 
/// 这个示例展示如何使用 bcrypt 创建用户并存储到数据库
use bcrypt::{hash, DEFAULT_COST};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!("=== RTMP 用户创建工具 ===\n");

    // 示例：创建管理员用户
    let username = "admin";
    let password = "admin123";
    let roles = vec!["admin".to_string()];

    // 使用 bcrypt 哈希密码
    let password_hash = hash(password, DEFAULT_COST)?;
    
    println!("用户名: {}", username);
    println!("密码: {}", password);
    println!("密码哈希: {}", password_hash);
    println!("角色: {:?}", roles);
    println!();

    // 验证哈希
    let is_valid = bcrypt::verify(password, &password_hash)?;
    println!("密码验证: {}", if is_valid { "✓ 成功" } else { "✗ 失败" });
    println!();

    // 生成 SQL 插入语句
    let user_id = uuid::Uuid::new_v4().to_string();
    let roles_json = serde_json::to_string(&roles)?;
    let created_at = chrono::Utc::now().to_rfc3339();

    println!("=== SQL 插入语句 ===");
    println!(
        "INSERT INTO rtmp_users (id, username, password_hash, roles, enabled, created_at) VALUES");
    println!(
        "  ('{}', '{}', '{}', '{}', true, '{}');",
        user_id, username, password_hash, roles_json, created_at
    );
    println!();

    // 其他示例用户
    println!("=== 其他示例用户 ===\n");
    
    for (user, pass, role_list) in [
        ("operator", "op123", vec!["operator"]),
        ("viewer", "view123", vec!["viewer"]),
    ] {
        let hash = hash(pass, DEFAULT_COST)?;
        let uid = uuid::Uuid::new_v4().to_string();
        let roles_str = serde_json::to_string(&role_list)?;
        
        println!("用户: {} / 密码: {}", user, pass);
        println!(
            "INSERT INTO rtmp_users (id, username, password_hash, roles, enabled, created_at) VALUES");
        println!(
            "  ('{}', '{}', '{}', '{}', true, '{}');",
            uid, user, hash, roles_str, created_at
        );
        println!();
    }

    Ok(())
}
