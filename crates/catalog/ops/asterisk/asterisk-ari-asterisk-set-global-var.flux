op asterisk-ari-asterisk-set-global-var(variable: String, value: String) -> Any
  description "Set the value of a global variable."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/asterisk/variable?variable={variable}")
  sep = "&"
  when value
    url = fmt("{url}{sep}value={value}")
  response = http.request(method: "POST", url)
  return response
