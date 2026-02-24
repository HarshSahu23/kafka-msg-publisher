import { Routes } from "@angular/router";
import { PublisherComponent } from "./publisher/publisher.component";

export const routes: Routes = [
  {
    path: '',
    component: PublisherComponent,
  },
  {
    path: 'shaper',
    loadComponent: () =>
      import('./shaper/shaper.component').then(m => m.ShaperComponent),
  },
];
