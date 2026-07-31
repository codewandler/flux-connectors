op twilio-call-get(account_sid: String, call_sid: String) -> Any
  description "Fetch one call"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.twilio.com/2010-04-01"
  url = fmt("{base}/Accounts/{account_sid}/Calls/{call_sid}.json")
  response = http.request(method: "GET", url)
  return response
