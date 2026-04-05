import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import * as sigilRaw from './generated/flashcards-domain';

type Card = { answer: string; id: string; prompt: string; references: Reference[]; title: string; topics: string[] };
type Reference = { kind: string; label: string; route: string };
type StudySession = { cards: Card[]; currentIndex: number; revealed: boolean; selectedTopic: string; topics: string[] };

type SigilDomain = {
  nextCard: (session: StudySession) => Promise<StudySession>;
  previousCard: (session: StudySession) => Promise<StudySession>;
  revealAnswer: (session: StudySession) => Promise<StudySession>;
  sessionForTopic: (selectedTopic: string) => Promise<StudySession>;
};

const sigil = sigilRaw as unknown as SigilDomain;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function siteBasePrefix(pathname: string): string {
  const marker = '/projects/sigil-flashcards/demo/';
  const index = pathname.indexOf(marker);
  if (index < 0) return '';
  const prefix = pathname.slice(0, index);
  return prefix.endsWith('/') ? prefix.slice(0, -1) : prefix;
}

function topicLabel(topic: string): string {
  if (topic === 'all') return 'All';
  return topic.charAt(0).toUpperCase() + topic.slice(1);
}

function FlashcardsApp(): JSX.Element {
  const [session, setSession] = useState<StudySession | null>(null);
  const [appError, setAppError] = useState<string | null>(null);
  const basePrefix = siteBasePrefix(window.location.pathname);

  useEffect(() => {
    void sigil.sessionForTopic('all')
      .then((nextSession) => {
        setSession(nextSession);
        setAppError(null);
      })
      .catch((error) => setAppError(errorMessage(error)));
  }, []);

  async function changeTopic(topic: string): Promise<void> {
    try {
      const nextSession = await sigil.sessionForTopic(topic);
      setSession(nextSession);
      setAppError(null);
    } catch (error) {
      setAppError(errorMessage(error));
    }
  }

  async function goNext(): Promise<void> {
    if (!session) return;
    try {
      setSession(await sigil.nextCard(session));
      setAppError(null);
    } catch (error) {
      setAppError(errorMessage(error));
    }
  }

  async function goPrevious(): Promise<void> {
    if (!session) return;
    try {
      setSession(await sigil.previousCard(session));
      setAppError(null);
    } catch (error) {
      setAppError(errorMessage(error));
    }
  }

  async function reveal(): Promise<void> {
    if (!session) return;
    try {
      setSession(await sigil.revealAnswer(session));
      setAppError(null);
    } catch (error) {
      setAppError(errorMessage(error));
    }
  }

  if (!session) {
    return (
      <div className="flashcards-shell">
        <p className="flashcards-banner">Loading curated Sigil deck...</p>
      </div>
    );
  }

  const currentCard = session.cards[session.currentIndex] ?? null;

  return (
    <div className="flashcards-shell">
      <header className="flashcards-header">
        <div>
          <p className="eyebrow">Projects / Sigil Flashcards</p>
          <h1>Sigil Flashcards</h1>
          <p className="subtitle">Curated cards for the questions humans ask when they want Codex or Claude Code to build with Sigil. The answer side links straight into the canonical docs, spec, and articles.</p>
        </div>
      </header>
      <section className="topic-strip" aria-label="Topic filters">
        {session.topics.map((topic) => (
          <button
            key={topic}
            type="button"
            className="topic-chip"
            data-active={session.selectedTopic === topic}
            onClick={() => void changeTopic(topic)}
          >
            {topicLabel(topic)}
          </button>
        ))}
      </section>
      {appError ? <p className="flashcards-banner error">App error: {appError}</p> : null}
      {currentCard ? (
        <>
          <section className="deck-meta">
            <span className="pill">{session.currentIndex + 1} / {session.cards.length}</span>
            <span className="pill muted">Topic: {topicLabel(session.selectedTopic)}</span>
            <span className="pill muted">{currentCard.topics.map(topicLabel).join(' · ')}</span>
          </section>
          <section className="flashcard" data-revealed={session.revealed}>
            <p className="card-label">{currentCard.title}</p>
            <h2>{currentCard.prompt}</h2>
            {session.revealed ? (
              <div className="answer-block">
                <p>{currentCard.answer}</p>
                <div className="reference-list">
                  {currentCard.references.map((reference) => (
                    <a className="reference-link" href={`${basePrefix}${reference.route}`} key={`${currentCard.id}-${reference.route}`}>
                      <span className="kind-badge">{reference.kind}</span>
                      <span>{reference.label}</span>
                    </a>
                  ))}
                </div>
              </div>
            ) : (
              <p className="hint">Reveal the answer to see the recommended Sigil feature choice and the canonical pages to read next.</p>
            )}
          </section>
          <section className="deck-controls">
            <button type="button" onClick={() => void goPrevious()}>Previous</button>
            <button type="button" className="primary" onClick={() => void reveal()} disabled={session.revealed}>Reveal answer</button>
            <button type="button" onClick={() => void goNext()}>Next</button>
          </section>
        </>
      ) : (
        <section className="flashcard empty">
          <h2>No cards for this topic yet.</h2>
          <p>Pick another topic to reopen the curated deck.</p>
        </section>
      )}
    </div>
  );
}

export function mountFlashcardsApp(element: HTMLElement): void {
  createRoot(element).render(<FlashcardsApp />);
}
