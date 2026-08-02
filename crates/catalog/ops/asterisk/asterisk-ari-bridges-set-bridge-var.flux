op asterisk-ari-bridges-set-bridge-var(bridgeId: String, variable: String, value: String, report_events: Bool) -> Any
  description "Set the value of a bridge variable or function."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}/variable?variable={variable}")
  sep = "&"
  when value
    url = fmt("{url}{sep}value={value}")
    sep = "&"
  when report_events
    url = fmt("{url}{sep}report_events={report_events}")
  response = http.request(method: "POST", url)
  return response
