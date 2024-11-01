import { initializeApp } from "firebase/app";
import { getAnalytics, isSupported, Analytics } from "firebase/analytics";

// Analytics works only in production mode and facets mode
let analytics: Analytics | null = null;

console.log(import.meta.env.MODE);

if (
  import.meta.env.MODE === "production" ||
  import.meta.env.MODE === "facets"
) {
  const firebaseConfig = {
    appId: import.meta.env.VITE_FIREBASE_APP_ID,
    measurementId: import.meta.env.VITE_FIREBASE_MEASUREMENT_ID,
    projectId: import.meta.env.VITE_FIREBASE_PROJECT_ID,
    apiKey: import.meta.env.VITE_FIREBASE_API_KEY,
  };

  console.log(firebaseConfig);

  const app = initializeApp(firebaseConfig);

  isSupported().then((supported) => {
    if (supported) {
      analytics = getAnalytics(app);
    } else {
      console.warn("Firebase Analytics is not supported in this environment.");
    }
  });
}
console.log(analytics);
export { analytics };
