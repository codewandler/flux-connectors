op confluence-comment-add(page_id: String, body: String) -> Any
  description "Add a footer comment to a page — the comment thread at the bottom, not an inline annotation on selected text. The comment is visible to everyone who can see the page and notifies its watchers; it cannot be restricted to a subset of them. The content is sent as Confluence storage format (`<p>text</p>`), not Markdown. Note the page is named in the body rather than in the path, and that this connector cannot read a page's existing comments back"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://{site}.atlassian.net/wiki"
  url = fmt("{base}/api/v2/footer-comments")
  content_type = "application/json"
  body_representation = "storage"
  payload = { body: { representation: body_representation, value: body }, pageId: page_id }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
