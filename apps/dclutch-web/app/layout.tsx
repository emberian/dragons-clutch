import type { Metadata } from 'next';
import { Geist, Geist_Mono } from 'next/font/google';
import './globals.css';
import './charts.css';

const geistSans = Geist({
  variable: '--font-geist-sans',
  subsets: ['latin'],
});

const geistMono = Geist_Mono({
  variable: '--font-geist-mono',
  subsets: ['latin'],
});

const TITLE = 'dClutch · Fully collateralized markets on Solana devnet';
const DESCRIPTION =
  'Buy claims on real-world outcomes, each one fully backed by collateral locked up before the claim exists. Deployed on Solana devnet: nothing is for sale and nothing is at risk.';

// Absolute URLs on purpose: share cards are read by scrapers against the
// public host, and a relative URL in a static export resolves to nothing
// when the document is fetched by a crawler that never runs the app.
const SITE_CARD = 'https://clutch.dregg.pro/og/site-card-v1.jpg';

export const metadata: Metadata = {
  metadataBase: new URL('https://clutch.dregg.pro'),
  title: TITLE,
  description: DESCRIPTION,
  openGraph: {
    title: TITLE,
    description: DESCRIPTION,
    siteName: 'dClutch',
    type: 'website',
    images: [{
      url: SITE_CARD,
      width: 1200,
      height: 630,
      alt: 'A dragon’s claw cradling a glowing faceted gem — the dClutch key art.',
    }],
  },
  twitter: {
    card: 'summary_large_image',
    title: TITLE,
    description: DESCRIPTION,
    images: [SITE_CARD],
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body
        className={`${geistSans.variable} ${geistMono.variable} antialiased`}
      >
        <a className="skip-link" href="#main-content">Skip to main content</a>
        {children}
      </body>
    </html>
  );
}
