op asterisk-ari-recordings-unpause(recordingName: String) -> Any
  description "Unpause a live recording."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/recordings/live/{recordingName}/pause")
  response = http.request(method: "DELETE", url)
  return response
