op asterisk-ari-channels-snoop-channel-with-id(channelId: String, snoopId: String, spy: String, whisper: String, app: String, appArgs: String) -> Any
  description "Start snooping."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/snoop/{snoopId}")
  response = http.request(method: "POST", query: { app, appArgs, spy, whisper }, url)
  return response
