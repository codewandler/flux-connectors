op asterisk-ari-recordings-get-live(recordingName: String) -> Any
  description "List live recordings."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/recordings/live/{recordingName}")
  response = http.request(method: "GET", url)
  return response
