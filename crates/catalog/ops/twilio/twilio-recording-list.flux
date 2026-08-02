op twilio-recording-list(IncludeSoftDeleted: Bool, PageSize: Number, Page: Number) -> Any
  description "List recordings for the configured Twilio account with bounded pagination"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.twilio.com/2010-04-01"
  AccountSid = "{username.twilio.basic_auth}"
  url = fmt("{base}/Accounts/{AccountSid}/Recordings.json")
  sep = "?"
  when IncludeSoftDeleted
    url = fmt("{url}{sep}IncludeSoftDeleted={IncludeSoftDeleted}")
    sep = "&"
  when PageSize
    url = fmt("{url}{sep}PageSize={PageSize}")
    sep = "&"
  when Page
    url = fmt("{url}{sep}Page={Page}")
  response = http.request(method: "GET", url)
  return response
