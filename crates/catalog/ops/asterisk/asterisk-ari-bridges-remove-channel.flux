op asterisk-ari-bridges-remove-channel(bridgeId: String, channel: List<String>) -> Any
  description "Remove a channel from a bridge."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}/removeChannel?channel={channel}")
  response = http.request(method: "POST", url)
  return response
