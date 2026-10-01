//! Test-only invariant streams. D-4 normalizes only an interaction's sibling
//! Timing/AttrBlock pair; all other nodes and tokens retain their order.

use ridl_syntax::{Profile, SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::{NodeOrToken, WalkEvent};

type Element = NodeOrToken<SyntaxNode, SyntaxToken>;

fn normalized_walk(node: &SyntaxNode, events: &mut Vec<WalkEvent<Element>>) {
    events.push(WalkEvent::Enter(NodeOrToken::Node(node.clone())));
    let mut children: Vec<_> = node.children_with_tokens().collect();
    if matches!(
        node.kind(),
        SyntaxKind::SignalDef
            | SyntaxKind::EventDef
            | SyntaxKind::FixedDef
            | SyntaxKind::CommandDef
            | SyntaxKind::QueryDef
    ) {
        let attributes = children
            .iter()
            .position(|e| e.kind() == SyntaxKind::AttrBlock);
        let timing = children.iter().position(|e| e.kind() == SyntaxKind::Timing);
        if let (Some(attributes), Some(timing)) = (attributes, timing)
            && attributes < timing
        {
            let timing = children.remove(timing);
            children.insert(attributes, timing);
        }
    }
    for child in children {
        match child {
            NodeOrToken::Node(child) => normalized_walk(&child, events),
            NodeOrToken::Token(token) => {
                events.push(WalkEvent::Enter(NodeOrToken::Token(token.clone())));
                events.push(WalkEvent::Leave(NodeOrToken::Token(token)));
            }
        }
    }
    events.push(WalkEvent::Leave(NodeOrToken::Node(node.clone())));
}

fn events(text: &str, profile: Profile) -> Vec<WalkEvent<Element>> {
    let parse = ridl_syntax::parse(text, profile);
    assert!(
        parse.errors().is_empty(),
        "invariant input must parse: {:?}",
        parse.errors()
    );
    let mut events = Vec::new();
    normalized_walk(&parse.syntax(), &mut events);
    events
}

pub(crate) fn content_tokens(text: &str, profile: Profile) -> Vec<(SyntaxKind, String)> {
    events(text, profile)
        .into_iter()
        .filter_map(|event| match event {
            WalkEvent::Enter(NodeOrToken::Token(token))
                if !matches!(token.kind(), SyntaxKind::Whitespace | SyntaxKind::Comma) =>
            {
                let text = if token.kind().is_trivia() {
                    token.text().trim_end().to_string()
                } else {
                    token.text().to_string()
                };
                Some((token.kind(), text))
            }
            _ => None,
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StructureEvent {
    Enter(SyntaxKind),
    Leave(SyntaxKind),
    Token(SyntaxKind, String),
}

pub(crate) fn syntax_structure(text: &str, profile: Profile) -> Vec<StructureEvent> {
    events(text, profile)
        .into_iter()
        .filter_map(|event| match event {
            WalkEvent::Enter(NodeOrToken::Node(node)) => Some(StructureEvent::Enter(node.kind())),
            WalkEvent::Leave(NodeOrToken::Node(node)) => Some(StructureEvent::Leave(node.kind())),
            WalkEvent::Enter(NodeOrToken::Token(token))
                if !token.kind().is_trivia() && token.kind() != SyntaxKind::Comma =>
            {
                Some(StructureEvent::Token(
                    token.kind(),
                    token.text().to_string(),
                ))
            }
            _ => None,
        })
        .collect()
}
