op klaviyo-profile-list -> Any
  description "List customer profiles in the account, newest first. Returns the FIRST PAGE ONLY — Klaviyo pages with an opaque `page[cursor]` query parameter this connector cannot send, so there is no way to reach later pages and no way to filter. Every profile carries personal data: an email address, often a phone number and a postal location"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://a.klaviyo.com/api"
  url = fmt("{base}/profiles")
  revision = "2026-07-15"
  response = http.request(headers: { revision }, method: "GET", url)
  return response
