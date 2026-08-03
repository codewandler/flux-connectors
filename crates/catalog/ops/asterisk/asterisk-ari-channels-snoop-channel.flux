op asterisk-ari-channels-snoop-channel(channelId: String, spy: String, whisper: String, app: String, appArgs: String, snoopId: String) -> Any
  description "Start snooping."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/snoop")
  response = http.request(method: "POST", query: { app, appArgs, snoopId, spy, whisper }, url)
  return response
