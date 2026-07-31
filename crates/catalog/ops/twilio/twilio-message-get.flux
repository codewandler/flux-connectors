op twilio-message-get(account_sid: String, message_sid: String) -> Any
  description "Fetch one message"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.twilio.com/2010-04-01"
  url = fmt("{base}/Accounts/{account_sid}/Messages/{message_sid}.json")
  response = http.request(method: "GET", url)
  return response
