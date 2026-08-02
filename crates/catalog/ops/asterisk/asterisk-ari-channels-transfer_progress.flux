op asterisk-ari-channels-transfer_progress(channelId: String, states: String) -> Any
  description "Inform the channel about the progress of the attended/blind transfer."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/transfer_progress?states={states}")
  response = http.request(method: "POST", url)
  return response
