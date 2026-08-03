op asterisk-ari-events-claim-channel(channelId: String, application: String) -> Any
  description "Claim a broadcast channel for this application."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/events/claim")
  response = http.request(method: "POST", query: { application, channelId }, url)
  return response
