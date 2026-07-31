op klaviyo-account-list -> Any
  description "Read the Klaviyo account this API key belongs to, with its timezone, currency, industry and contact details. Returns exactly one account — a private key is scoped to one — so this is the call that answers 'which account am I connected to'. Also this connector's verify: it takes no argument and needs only the accounts:read scope"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://a.klaviyo.com/api"
  url = fmt("{base}/accounts")
  revision = "2026-07-15"
  response = http.request(headers: { revision }, method: "GET", url)
  return response
