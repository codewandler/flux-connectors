op mailchimp-ping -> Any
  description "Check that the API is reachable and the credential works. Takes no argument and returns no account data — Mailchimp calls it a health check that returns nothing account-specific. Also this connector's `verify`: it is the one call that proves both halves of the configuration at once, because a wrong key and a wrong datacentre label both fail it"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{dc}.api.mailchimp.com/3.0"
  url = fmt("{base}/ping")
  response = http.request(method: "GET", url)
  return response
