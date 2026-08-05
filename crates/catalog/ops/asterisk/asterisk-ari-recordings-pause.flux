op asterisk-ari-recordings-pause(recordingName: String) -> Any
  description "Pause a live recording."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/recordings/live/{recordingName}/pause")
  response = http.request(method: "POST", url)
  return response
