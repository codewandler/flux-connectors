op asterisk-ari-channels-transfer_progress(channelId: String, states: String) -> Any
  description "Inform the channel about the progress of the attended/blind transfer."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/transfer_progress")
  response = http.request(method: "POST", query: { states }, url)
  return response
