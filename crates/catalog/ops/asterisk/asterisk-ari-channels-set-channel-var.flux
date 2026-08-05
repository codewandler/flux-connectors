op asterisk-ari-channels-set-channel-var(channelId: String, variable: String, value: String, report_events: Bool) -> Any
  description "Set the value of a channel variable or function."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/variable")
  response = http.request(method: "POST", query: { report_events, value, variable }, url)
  return response
