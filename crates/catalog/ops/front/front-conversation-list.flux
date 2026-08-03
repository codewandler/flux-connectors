op front-conversation-list(limit: Number) -> Any
  description "List the company's conversations in reverse chronological order (most recently updated first), first page only — this connector cannot follow Front's next-page link (see providers/front.toml's header comment). Front's structured search filter (`q`) is not accepted here: it is a bracket-notation object this pipeline cannot encode as a query string. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/_error/message`, its error code at `/_error/status` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api2.frontapp.com"
  url = fmt("{base}/conversations")
  response = http.request(method: "GET", query: { limit }, url)
  return response
