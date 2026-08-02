op asterisk-ari-recordings-get-stored(recordingName: String) -> Any
  description "Get a stored recording's details."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/recordings/stored/{recordingName}")
  response = http.request(method: "GET", url)
  return response
