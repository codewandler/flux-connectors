op asterisk-ari-playbacks-control(playbackId: String, operation: String) -> Any
  description "Control a playback."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/playbacks/{playbackId}/control")
  response = http.request(method: "POST", query: { operation }, url)
  return response
