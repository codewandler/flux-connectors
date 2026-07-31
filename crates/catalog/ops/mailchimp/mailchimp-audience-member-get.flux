op mailchimp-audience-member-get(list_id: String, subscriber_hash: String) -> Any
  description "Get one contact in an audience, including its subscription status and opt-in record. The contact is addressed by a hash of its own address, not by the address — see `subscriber_hash`. Returns personal data"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{dc}.api.mailchimp.com/3.0"
  url = fmt("{base}/lists/{list_id}/members/{subscriber_hash}")
  response = http.request(method: "GET", url)
  return response
