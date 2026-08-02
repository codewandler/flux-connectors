op asterisk-ari-channels-set-channel-var(channelId: String, variable: String, value: String, report_events: Bool) -> Any
  description "Set the value of a channel variable or function."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/variable?variable={variable}")
  sep = "&"
  when value
    url = fmt("{url}{sep}value={value}")
    sep = "&"
  when report_events
    url = fmt("{url}{sep}report_events={report_events}")
  response = http.request(method: "POST", url)
  return response
