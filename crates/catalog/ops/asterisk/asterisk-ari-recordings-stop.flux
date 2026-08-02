op asterisk-ari-recordings-stop(recordingName: String) -> Any
  description "Stop a live recording and store it."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/recordings/live/{recordingName}/stop")
  response = http.request(method: "POST", url)
  return response
