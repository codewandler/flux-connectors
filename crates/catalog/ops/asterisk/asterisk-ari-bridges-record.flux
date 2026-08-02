op asterisk-ari-bridges-record(bridgeId: String, name: String, format: String, recorder_format: String, maxDurationSeconds: Number, maxSilenceSeconds: Number, ifExists: String, beep: Bool, terminateOn: String) -> Any
  description "Start a recording."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/bridges/{bridgeId}/record?name={name}&format={format}")
  sep = "&"
  when recorder_format
    url = fmt("{url}{sep}recorder_format={recorder_format}")
    sep = "&"
  when maxDurationSeconds
    url = fmt("{url}{sep}maxDurationSeconds={maxDurationSeconds}")
    sep = "&"
  when maxSilenceSeconds
    url = fmt("{url}{sep}maxSilenceSeconds={maxSilenceSeconds}")
    sep = "&"
  when ifExists
    url = fmt("{url}{sep}ifExists={ifExists}")
    sep = "&"
  when beep
    url = fmt("{url}{sep}beep={beep}")
    sep = "&"
  when terminateOn
    url = fmt("{url}{sep}terminateOn={terminateOn}")
  response = http.request(method: "POST", url)
  return response
