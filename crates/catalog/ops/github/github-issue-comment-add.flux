op github-issue-comment-add(owner: String, repo: String, issue_number: Number, body: String) -> Any
  description "Add a comment to an issue or pull request. The comment is public to everyone who can see the repository and notifies its participants; GitHub has no private or internal comment here"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.github.com"
  url = fmt("{base}/repos/{owner}/{repo}/issues/{issue_number}/comments")
  content_type = "application/json"
  Accept = "application/vnd.github+json"
  payload = { body }
  response = http.request(body: payload, headers: { Accept, "content-type": content_type }, method: "POST", url)
  return response
