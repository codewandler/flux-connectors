op asterisk-ari-playbacks-control(playbackId: String, operation: String) -> Any
  description "Control a playback."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/playbacks/{playbackId}/control?operation={operation}")
  response = http.request(method: "POST", url)
  return response
