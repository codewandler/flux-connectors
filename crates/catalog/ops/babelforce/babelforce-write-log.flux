op babelforce-write-log(category: String, level: String, message: String, seq: Number, timestamp: Number, visible: Bool) -> Any
  description "Write a log entry"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/logs")
  content_type = "application/json"
  payload = { category, level, message, seq: $seq, timestamp, visible }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
