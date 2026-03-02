#!/bin/bash

# 生成用户并插入数据库
cd /Volumes/fushilu/workspace/flux-iot/crates/flux-rtmpd

# 运行示例工具并提取 SQL
output=$(cargo run --example create_user --features persistence 2>&1)

# 提取 admin 用户的 INSERT 语句
admin_sql=$(echo "$output" | grep -A 1 "INSERT INTO rtmp_users.*admin" | tail -1 | sed "s/');/');/")

# 提取 operator 用户的 INSERT 语句  
operator_sql=$(echo "$output" | grep -A 1 "INSERT INTO rtmp_users.*operator" | tail -1 | sed "s/');/');/")

# 提取 viewer 用户的 INSERT 语句
viewer_sql=$(echo "$output" | grep -A 1 "INSERT INTO rtmp_users.*viewer" | tail -1 | sed "s/');/');/")

# 执行插入
cd /Volumes/fushilu/workspace/flux-iot
sqlite3 data/rtmpd_users.db "$admin_sql"
sqlite3 data/rtmpd_users.db "$operator_sql"
sqlite3 data/rtmpd_users.db "$viewer_sql"

echo "✅ 用户已插入数据库"
sqlite3 data/rtmpd_users.db "SELECT username, enabled FROM rtmp_users;"
