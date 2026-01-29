import time
import json
import sys

try:
    import paho.mqtt.client as mqtt
except ImportError:
    print("paho-mqtt not installed. Please install: pip install paho-mqtt")
    sys.exit(1)

def on_connect(client, userdata, flags, rc):
    print(f"Connected with result code {rc}")
    # Publish a message that should trigger a rule
    # Rule: speed > 80 from previous dynamic rule
    payload = {"speed": 120}
    client.publish("sensor/speed", json.dumps(payload))
    print("Published to sensor/speed: ", payload)
    
    # Also publish to temp topic
    payload2 = {"value": 35.0} # > 30 triggers default rule
    client.publish("sensor/temp", json.dumps(payload2))
    print("Published to sensor/temp: ", payload2)
    
    client.disconnect()

# Use explicit client ID and protocol version if possible, or just string.
# Paho automatically generates one if empty, but let's be explicit.
client = mqtt.Client(client_id="flux_test_client")
client.on_connect = on_connect

client.loop_start()

print("Connecting to 127.0.0.1:1883...")
try:
    client.connect("127.0.0.1", 1883, 60)
except Exception as e:
    print(f"Connection failed: {e}")
    sys.exit(1)

# Wait enough time for connection and publishing (on_connect is async callback)
print("Waiting for publish...")
time.sleep(2) 

client.disconnect()
client.loop_stop()
print("Test completed.")
