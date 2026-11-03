// ZapZap's donation methods; the backend owns the URLs (donation.rs).
export type DonationMethod = { id: string; title: string; description: string; icon: string };

export const DONATION_METHODS: DonationMethod[] = [
  {
    id: "github_sponsors",
    title: "GitHub Sponsors",
    description: "Support ZapZap through GitHub and help fund its long-term development.",
    icon: "</>",
  },
  {
    id: "pix",
    title: "Pix",
    description: "Continue securely in your browser and complete the contribution with your banking app.",
    icon: "◆",
  },
  {
    id: "paypal",
    title: "PayPal",
    description: "A simple and secure way to support ZapZap from anywhere.",
    icon: "▭",
  },
  {
    id: "wise",
    title: "Wise",
    description: "International contributions with reduced fees and competitive exchange rates.",
    icon: "⇄",
  },
  {
    id: "kofi",
    title: "Ko-fi",
    description: "Make a one-time or recurring contribution and help keep the project active.",
    icon: "☕",
  },
];
