op asterisk-ari-channels-hangup(channelId: String, reason_code: String, reason: String) -> Any
  description "Delete (i.e. hangup) a channel."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}")
  sep = "?"
  when reason_code
    url = fmt("{url}{sep}reason_code={reason_code}")
    sep = "&"
  when reason
    url = fmt("{url}{sep}reason={reason}")
  response = http.request(method: "DELETE", url)
  return response
