use flux_video::ai::{VideoAnalyzer, AIConfig};
use std::fs;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== AI 视频分析测试 ===\n");

    // 示例：使用不同的云厂商 API
    
    // 1. 阿里云配置示例
    println!("1. 阿里云 AI 分析配置");
    let aliyun_config = AIConfig::aliyun(
        std::env::var("ALIYUN_ACCESS_KEY_ID").unwrap_or_else(|_| "your-key-id".to_string()),
        std::env::var("ALIYUN_ACCESS_KEY_SECRET").unwrap_or_else(|_| "your-key-secret".to_string()),
    );
    println!("   提供商: {}", aliyun_config.provider);
    println!("   端点: {}", aliyun_config.endpoint);
    println!();

    // 2. 腾讯云配置示例
    println!("2. 腾讯云 AI 分析配置");
    let tencent_config = AIConfig::tencent(
        std::env::var("TENCENT_SECRET_ID").unwrap_or_else(|_| "your-secret-id".to_string()),
        std::env::var("TENCENT_SECRET_KEY").unwrap_or_else(|_| "your-secret-key".to_string()),
    );
    println!("   提供商: {}", tencent_config.provider);
    println!("   端点: {}", tencent_config.endpoint);
    println!();

    // 3. AWS Rekognition 配置示例
    println!("3. AWS Rekognition 配置");
    let aws_config = AIConfig::aws(
        std::env::var("AWS_ACCESS_KEY").unwrap_or_else(|_| "your-access-key".to_string()),
        std::env::var("AWS_SECRET_KEY").unwrap_or_else(|_| "your-secret-key".to_string()),
        "us-east-1".to_string(),
    );
    println!("   提供商: {}", aws_config.provider);
    println!("   端点: {}", aws_config.endpoint);
    println!();

    // 4. 自定义 API 配置示例
    println!("4. 自定义 API 配置");
    let custom_config = AIConfig::custom(
        "https://your-ai-api.com/analyze".to_string(),
        std::env::var("CUSTOM_API_KEY").unwrap_or_else(|_| "your-api-key".to_string()),
    );
    println!("   提供商: {}", custom_config.provider);
    println!("   端点: {}", custom_config.endpoint);
    println!();

    // 创建分析器（使用自定义配置作为示例）
    let analyzer = VideoAnalyzer::new(custom_config);

    // 模拟视频帧（实际应用中从视频流获取）
    println!("5. 分析测试");
    println!("   注意: 需要配置有效的 API 密钥才能实际调用");
    println!();

    // 如果有测试图片，可以尝试分析
    if let Ok(frame_data) = fs::read("test_frame.jpg") {
        println!("   发现测试图片 test_frame.jpg");
        println!("   开始分析...");
        
        match analyzer.analyze_frame(&frame_data).await {
            Ok(result) => {
                println!("   ✅ 分析成功!");
                println!("   检测到 {} 个对象", result.objects.len());
                
                for (i, obj) in result.objects.iter().enumerate().take(5) {
                    println!("   [{}] {} (置信度: {:.2}%)", 
                        i + 1, obj.class, obj.confidence * 100.0);
                }
                
                if result.objects.len() > 5 {
                    println!("   ... 还有 {} 个对象", result.objects.len() - 5);
                }
            }
            Err(e) => {
                println!("   ❌ 分析失败: {}", e);
                println!("   提示: 检查 API 密钥和网络连接");
            }
        }
    } else {
        println!("   未找到测试图片 test_frame.jpg");
        println!("   跳过实际分析测试");
    }

    println!();
    println!("=== 测试完成 ===");
    println!();
    println!("💡 使用提示:");
    println!("   1. 设置环境变量配置 API 密钥");
    println!("   2. 选择合适的云厂商（阿里云/腾讯云/AWS）");
    println!("   3. 或使用自定义 API 端点");
    println!("   4. 准备测试图片 test_frame.jpg 进行测试");
    println!();
    println!("环境变量示例:");
    println!("   export ALIYUN_ACCESS_KEY_ID=your-key");
    println!("   export ALIYUN_ACCESS_KEY_SECRET=your-secret");
    println!("   export TENCENT_SECRET_ID=your-id");
    println!("   export TENCENT_SECRET_KEY=your-key");
    println!("   export AWS_ACCESS_KEY=your-key");
    println!("   export AWS_SECRET_KEY=your-secret");
    println!("   export CUSTOM_API_KEY=your-key");

    Ok(())
}
