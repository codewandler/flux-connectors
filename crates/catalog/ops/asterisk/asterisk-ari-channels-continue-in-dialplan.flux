op asterisk-ari-channels-continue-in-dialplan(channelId: String, context: String, extension: String, priority: Number, label: String) -> Any
  description "Exit application; continue execution in the dialplan."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/continue")
  response = http.request(method: "POST", query: { context, extension, label, priority }, url)
  return response
