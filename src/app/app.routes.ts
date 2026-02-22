import { Routes } from "@angular/router";
import { PublisherComponent } from "./publisher/publisher.component";

export const routes: Routes = [
  {
    path: '',
    component: PublisherComponent,
  },
  // Future route:
  // { path: 'shaper', component: ShaperComponent },
];
