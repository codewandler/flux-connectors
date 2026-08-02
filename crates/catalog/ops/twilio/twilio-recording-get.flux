op twilio-recording-get(Sid: String, IncludeSoftDeleted: Bool) -> Any
  description "Fetch one recording's metadata from the configured Twilio account"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.twilio.com/2010-04-01"
  AccountSid = "{username.twilio.basic_auth}"
  url = fmt("{base}/Accounts/{AccountSid}/Recordings/{Sid}.json")
  sep = "?"
  when IncludeSoftDeleted
    url = fmt("{url}{sep}IncludeSoftDeleted={IncludeSoftDeleted}")
  response = http.request(method: "GET", url)
  return response
