op asterisk-ari-recordings-cancel(recordingName: String) -> Any
  description "Stop a live recording and discard it."
  risk "destructive"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/recordings/live/{recordingName}")
  response = http.request(method: "DELETE", url)
  return response
