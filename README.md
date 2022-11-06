<div align="center">

# hello-rust

<details>
<summary>
Deployed to ??!?!?!
</summary>

https://eu-west-2.console.aws.amazon.com/dynamodbv2/home?region=eu-west-2#item-explorer?initialTagKey=&table=tasks

</details>

[![Rust](https://github.com/SG60/hello-rust/actions/workflows/rust.yml/badge.svg)](https://github.com/SG60/hello-rust/actions/workflows/rust.yml)

</div>

---

Uses Just (see the justfile) for running scripts etc.


https://eu-west-2.console.aws.amazon.com/dynamodbv2/home?region=eu-west-2#item-explorer?initialTagKey=&table=tasks

# DB Design

https://docs.rs/aws-sdk-dynamodb/latest/aws_sdk_dynamodb/
https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/GettingStarted.html
https://github.com/awslabs/aws-sdk-rust/tree/main/examples/dynamodb
https://docs.aws.amazon.com/sdk-for-rust/latest/dg/getting-started.html
https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/best-practices.html

The DB just stores sync settings and refresh tokens.

<table>
<caption>DB Example</caption>
<tr>
	<th scope="col" colspan=2>PK</th>
	<th scope="col" colspan=99999>Attributes</th>
</tr>
<tr>
	<th scope="col">userId</th>
	<th scope="col">SK (GSI-1-PK)</th>
	<th scope="col">data (GSI-1-SK)</th>
	<th scope="col" colspan=99999></th>
</tr>
<tbody>
	<tr><td rowSpan=0>notionUserId
	<tr><td rowspan=2>userDetails<th>notionUserId (data)<th>googleU<th>googleRefreshToken<th>notionRefreshToken
		<tr><td>notionU#notionUserId<td>googleU123456<td>asdfasefa<td>asdfasefa
	<tr><td rowspan=2>sync#0<th>timestamp (data)<th>notionDatabase<th>googleCalendar<th>notionTitleId<th>notionDoneId
		<tr><td>1667695936<td>asdfase<td>asdf3<td>flkjhs<td>asdfasefa
	<tr><td rowspan=2>sync#2<th>timestamp (data)<th>notionDatabase<th>googleCalendar<th>notionTitleId<th>notionDoneId
		<tr><td>1061395921<td>asdfase<td>asdf3<td>flkjhs<td>asdfasefa
</tbody>
</table>

Probably doesn't need rows for items. Maybe they can all just be stored in memory.

https://eu-west-2.console.aws.amazon.com/dynamodbv2/home?region=eu-west-2#item-explorer?initialTagKey=&table=tasks
