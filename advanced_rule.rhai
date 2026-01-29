
// Advanced Rule: Event Counter & Time Check
// Count how many times this rule runs.
// Also check if current minute is even.

let count = state_get("my_counter");
if count == () {
    count = 0;
}
count = count + 1;
state_set("my_counter", count);

print("Rule Triggered! Count: " + count);

let now = now_ms();
print("Current Time: " + now);

// Only trigger alert if count > 2
count > 2
